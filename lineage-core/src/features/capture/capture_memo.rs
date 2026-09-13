//! 入力1件を確定するユースケース。
//!
//! document の insert と lineage(link) の append を**同一トランザクション**で確定させる。
//! これを分けると、途中で失敗したときに hash-chain が切れる。

use anyhow::{Result, bail};

use crate::domain::capture::{CaptureContext, DocumentAsset, ImageAttachment};
use crate::domain::lineage::{LineageInput, LineageLedger, relation};
use crate::domain::meta::{MetaAssignment, MetaSource, auto_label, parse_meta_tags};
use crate::domain::ports::{CaptureStore, CaptureTx};
use crate::domain::shared::{Clock, Hasher, IdGenerator};

/// ローカル利用（単一利用者）の actor。クラウド接続では JWT の sub が入る。
pub const LOCAL_ACTOR: &str = "local";

/// 文脈が取れなかったときの lineage の source。
const SOURCE_KIND_MINOS: &str = "minos";
const SOURCE_ID_CAPTURE: &str = "capture";
const SOURCE_KIND_APP: &str = "app";
const TARGET_KIND_DOCUMENT: &str = "document";

pub struct CaptureMemoInput {
    pub workspace_id: String,
    pub workspace_name: String,
    pub body: String,
    /// Some の場合は新規作成せず、選択した過去メモへ追記する。
    pub document_id: Option<String>,
    /// 入力欄のバッジとして確定済みのメタ情報（自動付与ぶんも含む）。
    ///
    /// 自動付与を並べるのは入力欄の役目なので、ここに載っていない自動メタ情報は
    /// 利用者が外したものとして扱い、記録には残さない。
    pub metas: Vec<MetaAssignment>,
    /// 直前に開いていたアプリケーションの情報（取得できた場合）。lineage の source になる。
    pub context: Option<CaptureContext>,
    /// このメモに紐付ける画像。画像自身も document として lineage に記録する。
    pub images: Vec<ImageAttachment>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaptureMemoOutput {
    pub document_id: String,
    pub title: String,
    /// 付与されたメタ情報のラベル（自動・ユーザ入力の両方）。
    pub meta_labels: Vec<String>,
    /// 追記された link の連番と content_hash。
    pub seq: i64,
    pub content_hash: String,
}

pub struct CaptureMemo<'a> {
    store: &'a dyn CaptureStore,
    clock: &'a dyn Clock,
    ids: &'a dyn IdGenerator,
    hasher: &'a dyn Hasher,
}

impl<'a> CaptureMemo<'a> {
    pub fn new(
        store: &'a dyn CaptureStore,
        clock: &'a dyn Clock,
        ids: &'a dyn IdGenerator,
        hasher: &'a dyn Hasher,
    ) -> Self {
        Self {
            store,
            clock,
            ids,
            hasher,
        }
    }

    pub fn execute(&self, input: CaptureMemoInput) -> Result<CaptureMemoOutput> {
        let body = input.body.trim_end().to_string();
        if body.trim().is_empty() {
            bail!("本文が空です");
        }

        let now = self.clock.now_rfc3339();
        let document = DocumentAsset::memo(
            input
                .document_id
                .clone()
                .unwrap_or_else(|| self.ids.new_id()),
            &input.workspace_id,
            body,
            &now,
        );
        let mut metas = collect_metas(&document.body_text, &input.metas);
        // `#app` is an explicit request to promote observed application metadata.
        if let Some(context) = input.context.as_ref()
            && metas.iter().any(|m| m.label == auto_label::APP)
        {
            if let Some(app) = metas.iter_mut().find(|m| m.label == auto_label::APP) {
                *app = MetaAssignment::derived(auto_label::APP, &context.process_name);
            }
        }

        // 自動付与の app が取れていれば、その文脈を lineage の source にする。
        let (source_kind, source_id) = match input.context.as_ref() {
            Some(context) => (SOURCE_KIND_APP, context.process_name.clone()),
            None => (SOURCE_KIND_MINOS, SOURCE_ID_CAPTURE.to_string()),
        };

        let lineage_input = LineageInput {
            workspace_id: input.workspace_id.clone(),
            source_kind: source_kind.to_string(),
            source_id,
            target_kind: TARGET_KIND_DOCUMENT.to_string(),
            target_id: document.id.clone(),
            relation_type: relation::DERIVED_FROM.to_string(),
            actor: LOCAL_ACTOR.to_string(),
            created_at: now.clone(),
        };

        let mut appended: Option<(i64, String)> = None;
        let images = input
            .images
            .into_iter()
            .map(|image| DocumentAsset::image(self.ids.new_id(), &input.workspace_id, image, &now))
            .collect::<Vec<_>>();

        self.store.transact(&mut |tx: &mut dyn CaptureTx| {
            tx.ensure_workspace(&input.workspace_id, &input.workspace_name, &now)?;
            if input.document_id.is_some() {
                tx.update_document(&document)?;
                tx.clear_document_metas(&document.id)?;
            } else {
                tx.insert_document(&document)?;
            }

            if let Some(context) = input.context.as_ref() {
                for metadata in context.metadata() {
                    tx.insert_document_metadata(&self.ids.new_id(), &document.id, &metadata, &now)?;
                }
            }

            for meta in &metas {
                // Observed metadata never reaches the completion registry.
                tx.learn_meta_tag(&self.ids.new_id(), &input.workspace_id, &meta.label, &now)?;
                tx.insert_document_meta(&self.ids.new_id(), &document.id, meta, &now)?;
            }

            let prev = tx.last_link(&input.workspace_id)?;
            let ledger = LineageLedger::new(self.hasher);
            let link = ledger.append_next(prev.as_ref(), self.ids.new_id(), lineage_input.clone());
            tx.append_link(&link)?;
            let mut previous = link;

            for image in &images {
                tx.insert_document(image)?;
                let attachment_input = LineageInput {
                    workspace_id: input.workspace_id.clone(),
                    source_kind: TARGET_KIND_DOCUMENT.to_string(),
                    source_id: image.id.clone(),
                    target_kind: TARGET_KIND_DOCUMENT.to_string(),
                    target_id: document.id.clone(),
                    relation_type: relation::ATTACHMENT_FOR.to_string(),
                    actor: LOCAL_ACTOR.to_string(),
                    created_at: now.clone(),
                };
                let attachment_link =
                    ledger.append_next(Some(&previous), self.ids.new_id(), attachment_input);
                tx.append_link(&attachment_link)?;
                previous = attachment_link;
            }

            appended = Some((previous.seq, previous.content_hash.clone()));
            Ok(())
        })?;

        let (seq, content_hash) =
            appended.ok_or_else(|| anyhow::anyhow!("lineage が追記されませんでした"))?;

        Ok(CaptureMemoOutput {
            document_id: document.id,
            title: document.title,
            meta_labels: metas.into_iter().map(|m| m.label).collect(),
            seq,
            content_hash,
        })
    }
}

/// 確定済みのバッジと、本文中に残った `#タグ` をまとめる。
///
/// 同じラベルが両方に現れた場合はユーザ入力を優先する（自動値で上書きしない）。
fn collect_metas(body: &str, confirmed: &[MetaAssignment]) -> Vec<MetaAssignment> {
    let mut metas = confirmed.to_vec();
    for meta in parse_meta_tags(body) {
        match metas
            .iter_mut()
            .find(|existing| existing.label == meta.label)
        {
            Some(existing) if existing.source == MetaSource::Auto => *existing = meta,
            Some(_) => {}
            None => metas.push(meta),
        }
    }
    metas
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::lineage::{LineageLedger, VerifyResult};
    use crate::domain::ports::MemoQuery;
    use crate::infra::clock::{FixedClock, SequentialIds};
    use crate::infra::crypto::Sha256Hasher;
    use crate::infra::sqlite::Database;

    struct Fixture {
        db: Database,
        clock: FixedClock,
        ids: SequentialIds,
        hasher: Sha256Hasher,
    }

    impl Fixture {
        fn new() -> Self {
            Self {
                db: Database::open_in_memory().unwrap(),
                clock: FixedClock::new("2026-08-08T12:00:00Z"),
                ids: SequentialIds::new(),
                hasher: Sha256Hasher,
            }
        }

        /// 入力欄と同じように、文脈から作った自動メタ情報をバッジとして渡す。
        fn capture(&self, body: &str, context: Option<CaptureContext>) -> CaptureMemoOutput {
            let metas = Vec::new();
            CaptureMemo::new(&self.db, &self.clock, &self.ids, &self.hasher)
                .execute(CaptureMemoInput {
                    workspace_id: "ws".into(),
                    workspace_name: "minos".into(),
                    body: body.into(),
                    document_id: None,
                    metas,
                    context,
                    images: Vec::new(),
                })
                .unwrap()
        }
    }

    #[test]
    fn stores_the_memo_with_its_metadata() {
        let f = Fixture::new();
        let context = CaptureContext {
            process_name: "chrome.exe".into(),
            window_title: "SOXL".into(),
        };

        let out = f.capture("SOXL 損切り #投資", Some(context));

        assert_eq!(out.title, "SOXL 損切り #投資");
        assert_eq!(out.meta_labels, vec!["投資"]);
        assert_eq!(out.seq, 1);
    }

    #[test]
    fn auto_metadata_removed_from_the_input_stays_out_of_the_record() {
        let f = Fixture::new();
        let out = CaptureMemo::new(&f.db, &f.clock, &f.ids, &f.hasher)
            .execute(CaptureMemoInput {
                workspace_id: "ws".into(),
                workspace_name: "minos".into(),
                body: "アプリ情報を外したメモ".into(),
                document_id: None,
                metas: Vec::new(),
                context: Some(CaptureContext {
                    process_name: "chrome.exe".into(),
                    window_title: "SOXL".into(),
                }),
                images: Vec::new(),
            })
            .unwrap();

        assert!(out.meta_labels.is_empty());
    }

    #[test]
    fn stores_confirmed_badges_without_putting_them_in_the_body() {
        let f = Fixture::new();
        let out = CaptureMemo::new(&f.db, &f.clock, &f.ids, &f.hasher)
            .execute(CaptureMemoInput {
                workspace_id: "ws".into(),
                workspace_name: "minos".into(),
                body: "バッジ付きのメモ".into(),
                document_id: None,
                metas: vec![MetaAssignment::user("タスク", None)],
                context: None,
                images: Vec::new(),
            })
            .unwrap();

        assert_eq!(out.title, "バッジ付きのメモ");
        assert_eq!(out.meta_labels, vec!["タスク"]);
    }

    #[test]
    fn extends_the_hash_chain_on_every_capture() {
        let f = Fixture::new();
        let first = f.capture("1件目", None);
        let second = f.capture("2件目", None);

        assert_eq!(first.seq, 1);
        assert_eq!(second.seq, 2);
        assert_ne!(first.content_hash, second.content_hash);

        let records = crate::domain::ports::LineageQuery::list(&f.db, "ws").unwrap();
        assert_eq!(records[1].prev_hash, records[0].content_hash);
        assert_eq!(
            LineageLedger::new(&f.hasher).verify(&records),
            VerifyResult::Ok { checked: 2 }
        );
    }

    #[test]
    fn stores_images_as_attachments_in_the_same_hash_chain() {
        let f = Fixture::new();
        let out = CaptureMemo::new(&f.db, &f.clock, &f.ids, &f.hasher)
            .execute(CaptureMemoInput {
                workspace_id: "ws".into(),
                workspace_name: "minos".into(),
                body: "画像付きメモ".into(),
                document_id: None,
                metas: Vec::new(),
                context: None,
                images: vec![ImageAttachment {
                    name: "chart.png".into(),
                    blob_uri: "/attachments/chart.png".into(),
                }],
            })
            .unwrap();

        let records = crate::domain::ports::LineageQuery::list(&f.db, "ws").unwrap();
        assert_eq!(out.seq, 2);
        assert_eq!(records[1].relation_type, relation::ATTACHMENT_FOR);
        assert_eq!(records[1].target_id, out.document_id);
        assert_eq!(records[1].prev_hash, records[0].content_hash);
    }

    #[test]
    fn rejects_an_empty_body() {
        let f = Fixture::new();
        let result =
            CaptureMemo::new(&f.db, &f.clock, &f.ids, &f.hasher).execute(CaptureMemoInput {
                workspace_id: "ws".into(),
                workspace_name: "minos".into(),
                body: "   \n ".into(),
                document_id: None,
                metas: Vec::new(),
                context: None,
                images: Vec::new(),
            });
        assert!(result.is_err());
    }

    #[test]
    fn explicit_app_promotes_observed_application_as_derived_tag() {
        let f = Fixture::new();
        let context = CaptureContext {
            process_name: "chrome.exe".into(),
            window_title: "T".into(),
        };
        let out = f.capture("#app=手入力 のメモ", Some(context));

        assert_eq!(out.meta_labels, vec!["app"]);
        let metas = f.db.metas_of_document(&out.document_id).unwrap();
        let app = metas.iter().find(|m| m.0 == "app").unwrap();
        assert_eq!(app.1.as_deref(), Some("chrome.exe"));
        assert_eq!(app.2, "derived");
    }

    #[test]
    fn learns_tags_for_completion() {
        let f = Fixture::new();
        f.capture("#タスク A", None);
        f.capture("#タスク B", None);
        f.capture("#投資 C", None);

        let tags = crate::domain::ports::MetaTagQuery::all(&f.db, "ws", 100).unwrap();
        let task = tags.iter().find(|t| t.label == "タスク").unwrap();
        assert_eq!(task.usage_count, 2);
        assert_eq!(task.last_used_at.as_deref(), Some("2026-08-08T12:00:00Z"));
    }

    #[test]
    fn updates_an_existing_memo_with_restored_tags() {
        let f = Fixture::new();
        let first = f.capture("最初の行", None);
        CaptureMemo::new(&f.db, &f.clock, &f.ids, &f.hasher)
            .execute(CaptureMemoInput {
                workspace_id: "ws".into(),
                workspace_name: "minos".into(),
                body: "最初の行\n追記した行".into(),
                document_id: Some(first.document_id.clone()),
                metas: vec![MetaAssignment::user("継続", None)],
                context: None,
                images: Vec::new(),
            })
            .unwrap();

        let restored = f.db.get("ws", &first.document_id).unwrap().unwrap();
        assert_eq!(restored.body_text, "最初の行\n追記した行");
        assert_eq!(restored.metas, vec![MetaAssignment::user("継続", None)]);
        assert_eq!(f.db.recent("ws", 8).unwrap()[0].id, first.document_id);
    }
}
