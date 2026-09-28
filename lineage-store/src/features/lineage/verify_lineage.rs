//! hash-chain の検証（真正性チェック）。
//!
//! ローカル(minos)でもクラウド(Workers)でも同じ `LineageLedger::verify` を使う。

use anyhow::Result;

use crate::ports::LineageQuery;
use lineage_core::domain::lineage::{LineageLedger, VerifyResult};
use lineage_core::domain::shared::Hasher;

pub struct VerifyLineage<'a> {
    lineage: &'a dyn LineageQuery,
    hasher: &'a dyn Hasher,
}

impl<'a> VerifyLineage<'a> {
    pub fn new(lineage: &'a dyn LineageQuery, hasher: &'a dyn Hasher) -> Self {
        Self { lineage, hasher }
    }

    pub fn execute(&self, workspace_id: &str) -> Result<VerifyResult> {
        let records = self.lineage.list(workspace_id)?;
        Ok(LineageLedger::new(self.hasher).verify(&records))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::document::{SaveDocument, SaveDocumentInput, WriteMode};
    use crate::infra::clock::{FixedClock, SequentialIds};
    use crate::infra::crypto::Sha256Hasher;
    use crate::infra::sqlite::Database;
    use crate::ports::{Clock, IdGenerator};
    use lineage_core::domain::document::DocumentAsset;
    use lineage_core::domain::lineage::BrokenReason;
    use lineage_core::domain::lineage::LineageInput;

    #[test]
    fn detects_a_tampered_ledger() {
        let db = Database::open_in_memory().unwrap();
        let clock = FixedClock::new("2026-08-08T12:00:00Z");
        let ids = SequentialIds::new();
        let hasher = Sha256Hasher;

        for body in ["1件目", "2件目", "3件目"] {
            let id = ids.new_id();
            let now = clock.now_rfc3339();
            SaveDocument::new(&db, &ids, &hasher)
                .execute(SaveDocumentInput {
                    workspace_name: "test".into(),
                    mode: WriteMode::Insert,
                    document: DocumentAsset {
                        id: id.clone(),
                        workspace_id: "ws".into(),
                        title: "Explicit title".into(),
                        body_text: body.into(),
                        blob_uri: None,
                        document_type: "memo".into(),
                        created_at: now.clone(),
                        updated_at: now.clone(),
                    },
                    lineage: LineageInput {
                        workspace_id: "ws".into(),
                        source_kind: "test".into(),
                        source_id: "seed".into(),
                        target_kind: "document".into(),
                        target_id: id,
                        relation_type: "derived_from".into(),
                        actor: "test".into(),
                        created_at: now,
                    },
                    metas: vec![],
                    metadata: vec![],
                    attachments: vec![],
                })
                .unwrap();
        }

        assert!(
            VerifyLineage::new(&db, &hasher)
                .execute("ws")
                .unwrap()
                .is_ok()
        );

        // 台帳を直接書き換える（＝改ざん）。
        db.force_update_link_actor_for_test("ws", 2, "someone-else")
            .unwrap();

        assert_eq!(
            VerifyLineage::new(&db, &hasher).execute("ws").unwrap(),
            VerifyResult::Broken {
                broken_at: 2,
                reason: BrokenReason::ContentHashMismatch,
            }
        );
    }
}
