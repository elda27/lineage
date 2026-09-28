//! Minos の入力モデル。タイトル・観測文脈をここで解釈し、共有記録へ明示的に変換する。

use anyhow::{Result, ensure};
use lineage_core::domain::document::{DOCUMENT_TYPE_IMAGE, DOCUMENT_TYPE_MEMO, DocumentAsset};
use lineage_core::domain::meta::DocumentMetadata;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Note {
    pub title: String,
    pub body: String,
}

impl Note {
    pub fn new(body: String) -> Result<Self> {
        let body = body.trim_end().to_string();
        ensure!(!body.trim().is_empty(), "本文が空です");
        Ok(Self {
            title: derive_title(&body),
            body,
        })
    }

    pub fn into_document(self, id: String, workspace_id: String, now: String) -> DocumentAsset {
        DocumentAsset {
            id,
            workspace_id,
            title: self.title,
            body_text: self.body,
            blob_uri: None,
            document_type: DOCUMENT_TYPE_MEMO.into(),
            created_at: now.clone(),
            updated_at: now,
        }
    }
}

/// minos でメモに添付する画像。`blob_uri` はローカルに保存した画像のパス。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageAttachment {
    pub name: String,
    pub blob_uri: String,
}

impl ImageAttachment {
    pub fn into_document(
        self,
        id: impl Into<String>,
        workspace_id: impl Into<String>,
        now: impl Into<String>,
    ) -> DocumentAsset {
        let now = now.into();
        DocumentAsset {
            id: id.into(),
            workspace_id: workspace_id.into(),
            title: self.name,
            body_text: String::new(),
            blob_uri: Some(self.blob_uri),
            document_type: DOCUMENT_TYPE_IMAGE.to_string(),
            created_at: now.clone(),
            updated_at: now,
        }
    }
}

/// 入力時に minos が観測した文脈（直前に開いていたアプリケーション）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaptureContext {
    /// 直前にフォアグラウンドだったアプリの実行ファイル名（例: `chrome.exe`）。
    pub process_name: String,
    /// そのウィンドウタイトル。
    pub window_title: String,
}

impl CaptureContext {
    /// 文脈から自動付与するメタ情報を作る。
    pub fn metadata(&self) -> Vec<DocumentMetadata> {
        let mut metas = vec![DocumentMetadata {
            key: "application".into(),
            value: self.process_name.clone(),
            source: "auto".into(),
        }];
        if !self.window_title.trim().is_empty() {
            metas.push(DocumentMetadata {
                key: "window".into(),
                value: self.window_title.clone(),
                source: "auto".into(),
            });
        }
        metas
    }
}

const TITLE_MAX_CHARS: usize = 60;

fn derive_title(body: &str) -> String {
    let first_line = body
        .lines()
        .find(|line| !line.trim().is_empty())
        .unwrap_or("");
    let trimmed = first_line.trim();
    if trimmed.is_empty() {
        return "memo".to_string();
    }

    let mut title: String = trimmed.chars().take(TITLE_MAX_CHARS).collect();
    if trimmed.chars().count() > TITLE_MAX_CHARS {
        title.push('…');
    }
    title
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn note_maps_the_first_nonblank_line_to_the_canonical_title() {
        let note = Note::new("\n\nSOXL 損切り\n理由は…\n".into()).unwrap();
        let doc = note.into_document("d1".into(), "ws".into(), "2026-08-08T00:00:00Z".into());
        assert_eq!(doc.title, "SOXL 損切り");
        assert_eq!(doc.body_text, "\n\nSOXL 損切り\n理由は…");
        assert_eq!(doc.document_type, DOCUMENT_TYPE_MEMO);
    }

    #[test]
    fn long_titles_are_elided_by_character() {
        let note = Note::new("あ".repeat(100)).unwrap();
        assert_eq!(note.title.chars().count(), TITLE_MAX_CHARS + 1);
        assert!(note.title.ends_with('…'));
    }

    #[test]
    fn empty_input_is_rejected() {
        assert!(Note::new(" \n ".into()).is_err());
    }

    #[test]
    fn context_becomes_observed_metadata() {
        let context = CaptureContext {
            process_name: "chrome.exe".into(),
            window_title: "SOXL".into(),
        };
        let metas = context.metadata();
        assert_eq!(metas[0].key, "application");
        assert_eq!(metas[0].value, "chrome.exe");
        assert_eq!(metas[1].key, "window");
    }
}

/// 自動付与するメタ情報のラベル。
pub mod auto_label {
    /// 直前に開いていたアプリケーションの実行ファイル名。
    pub const APP: &str = "app";
    /// 直前に開いていたウィンドウのタイトル。
    pub const WINDOW: &str = "window";
}
