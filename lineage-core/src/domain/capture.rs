//! minos が記録する「入力1件」＝ Document Asset。
//!
//! 利用者にとってはメモだが、内部的には Document として保存する
//! （docs/concept/PARTIAL_SPEC.md「3.3 文書ではなく記録として扱う」）。

use crate::domain::meta::DocumentMetadata;

/// document_type の値。
pub const DOCUMENT_TYPE_MEMO: &str = "memo";
pub const DOCUMENT_TYPE_IMAGE: &str = "image";

/// minos でメモに添付する画像。`blob_uri` はローカルに保存した画像のパス。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageAttachment {
    pub name: String,
    pub blob_uri: String,
}

/// 記録本体。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentAsset {
    pub id: String,
    pub workspace_id: String,
    pub title: String,
    pub body_text: String,
    pub blob_uri: Option<String>,
    pub document_type: String,
    pub created_at: String,
    pub updated_at: String,
}

impl DocumentAsset {
    /// 本文からタイトルを導出して記録を作る。
    ///
    /// 利用者にタイトルを入力させないため、1行目を要約として使う。
    pub fn memo(
        id: impl Into<String>,
        workspace_id: impl Into<String>,
        body_text: impl Into<String>,
        now: impl Into<String>,
    ) -> Self {
        let body_text = body_text.into();
        let now = now.into();
        Self {
            id: id.into(),
            workspace_id: workspace_id.into(),
            title: derive_title(&body_text),
            body_text,
            blob_uri: None,
            document_type: DOCUMENT_TYPE_MEMO.to_string(),
            created_at: now.clone(),
            updated_at: now,
        }
    }

    pub fn image(
        id: impl Into<String>,
        workspace_id: impl Into<String>,
        attachment: ImageAttachment,
        now: impl Into<String>,
    ) -> Self {
        let now = now.into();
        Self {
            id: id.into(),
            workspace_id: workspace_id.into(),
            title: attachment.name,
            body_text: String::new(),
            blob_uri: Some(attachment.blob_uri),
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
    fn title_comes_from_the_first_non_empty_line() {
        let doc = DocumentAsset::memo(
            "d1",
            "ws",
            "\n\nSOXL 損切り\n理由は…",
            "2026-08-08T00:00:00Z",
        );
        assert_eq!(doc.title, "SOXL 損切り");
        assert_eq!(doc.document_type, DOCUMENT_TYPE_MEMO);
    }

    #[test]
    fn long_titles_are_elided() {
        let body = "あ".repeat(100);
        let doc = DocumentAsset::memo("d1", "ws", body, "2026-08-08T00:00:00Z");
        assert_eq!(doc.title.chars().count(), TITLE_MAX_CHARS + 1);
        assert!(doc.title.ends_with('…'));
    }

    #[test]
    fn empty_body_falls_back_to_a_placeholder_title() {
        let doc = DocumentAsset::memo("d1", "ws", "   ", "2026-08-08T00:00:00Z");
        assert_eq!(doc.title, "memo");
    }

    #[test]
    fn context_becomes_auto_metadata() {
        let context = CaptureContext {
            process_name: "chrome.exe".into(),
            window_title: "SOXL - Google Finance".into(),
        };
        let metas = context.metadata();
        assert_eq!(metas[0].key, "application");
        assert_eq!(metas[0].value, "chrome.exe");
        assert_eq!(metas[1].key, "window");
    }
}
