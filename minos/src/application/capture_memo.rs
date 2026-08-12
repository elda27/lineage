//! 入力1件を確定するユースケース。
//!
//! document の insert と lineage(link) の append を**同一トランザクション**で確定させる。
//! これを分けると、途中で失敗したときに hash-chain が切れる。

use anyhow::{Result, bail};

use crate::domain::capture::{CaptureContext, DocumentAsset};
use crate::domain::lineage::{LineageInput, LineageLedger, relation};
use crate::domain::meta::{MetaAssignment, parse_meta_tags};
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
    /// 入力欄で確定済みのユーザタグ。
    pub user_metas: Vec<MetaAssignment>,
    /// 直前に開いていたアプリケーションの情報（取得できた場合）。
    pub context: Option<CaptureContext>,
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
        let document = DocumentAsset::memo(self.ids.new_id(), &input.workspace_id, body, &now);
        let metas = collect_metas(
            &document.body_text,
            &input.user_metas,
            input.context.as_ref(),
        );

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

        self.store.transact(&mut |tx: &mut dyn CaptureTx| {
            tx.ensure_workspace(&input.workspace_id, &input.workspace_name, &now)?;
            tx.insert_document(&document)?;

            for meta in &metas {
                tx.insert_document_meta(&self.ids.new_id(), &document.id, meta, &now)?;
                tx.learn_meta_tag(
                    &self.ids.new_id(),
                    &input.workspace_id,
                    &meta.label,
                    &now,
                )?;
            }

            let prev = tx.last_link(&input.workspace_id)?;
            let ledger = LineageLedger::new(self.hasher);
            let link = ledger.append_next(prev.as_ref(), self.ids.new_id(), lineage_input.clone());
            tx.append_link(&link)?;

            appended = Some((link.seq, link.content_hash.clone()));
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

/// 自動付与のメタ情報と、本文中の `#タグ` をまとめる。
///
/// 同じラベルが両方に現れた場合はユーザ入力を優先する（自動値で上書きしない）。
fn collect_metas(
    body: &str,
    user_metas: &[MetaAssignment],
    context: Option<&CaptureContext>,
) -> Vec<MetaAssignment> {
    let mut metas = user_metas.to_vec();
    for meta in parse_meta_tags(body) {
        if !metas.iter().any(|existing| existing.label == meta.label) {
            metas.push(meta);
        }
    }
    if let Some(context) = context {
        for auto in context.auto_metas() {
            if !metas.iter().any(|m| m.label == auto.label) {
                metas.push(auto);
            }
        }
    }
    metas
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::lineage::{LineageLedger, VerifyResult};
    use crate::infrastructure::clock::{FixedClock, SequentialIds};
    use crate::infrastructure::crypto::Sha256Hasher;
    use crate::infrastructure::sqlite::Database;

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

        fn capture(&self, body: &str, context: Option<CaptureContext>) -> CaptureMemoOutput {
            CaptureMemo::new(&self.db, &self.clock, &self.ids, &self.hasher)
                .execute(CaptureMemoInput {
                    workspace_id: "ws".into(),
                    workspace_name: "minos".into(),
                    body: body.into(),
                    user_metas: Vec::new(),
                    context,
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
        assert_eq!(out.meta_labels, vec!["投資", "app", "window"]);
        assert_eq!(out.seq, 1);
    }

    #[test]
    fn stores_confirmed_badges_without_putting_them_in_the_body() {
        let f = Fixture::new();
        let out = CaptureMemo::new(&f.db, &f.clock, &f.ids, &f.hasher)
            .execute(CaptureMemoInput {
                workspace_id: "ws".into(),
                workspace_name: "minos".into(),
                body: "バッジ付きのメモ".into(),
                user_metas: vec![MetaAssignment::user("タスク", None)],
                context: None,
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
    fn rejects_an_empty_body() {
        let f = Fixture::new();
        let result = CaptureMemo::new(&f.db, &f.clock, &f.ids, &f.hasher).execute(CaptureMemoInput {
            workspace_id: "ws".into(),
            workspace_name: "minos".into(),
            body: "   \n ".into(),
            user_metas: Vec::new(),
            context: None,
        });
        assert!(result.is_err());
    }

    #[test]
    fn a_user_tag_wins_over_the_auto_value_of_the_same_label() {
        let f = Fixture::new();
        let context = CaptureContext {
            process_name: "chrome.exe".into(),
            window_title: "T".into(),
        };
        let out = f.capture("#app=手入力 のメモ", Some(context));

        assert_eq!(out.meta_labels, vec!["app", "window"]);
        let metas = f.db.metas_of_document(&out.document_id).unwrap();
        let app = metas.iter().find(|m| m.0 == "app").unwrap();
        assert_eq!(app.1.as_deref(), Some("手入力"));
        assert_eq!(app.2, "user");
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
}
