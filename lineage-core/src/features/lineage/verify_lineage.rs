//! hash-chain の検証（真正性チェック）。
//!
//! ローカル(minos)でもクラウド(Workers)でも同じ `LineageLedger::verify` を使う。

use anyhow::Result;

use crate::domain::lineage::{LineageLedger, VerifyResult};
use crate::domain::ports::LineageQuery;
use crate::domain::shared::Hasher;

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
    use crate::domain::lineage::BrokenReason;
    use crate::features::capture::{CaptureMemo, CaptureMemoInput};
    use crate::infra::clock::{FixedClock, SequentialIds};
    use crate::infra::crypto::Sha256Hasher;
    use crate::infra::sqlite::Database;

    #[test]
    fn detects_a_tampered_ledger() {
        let db = Database::open_in_memory().unwrap();
        let clock = FixedClock::new("2026-08-08T12:00:00Z");
        let ids = SequentialIds::new();
        let hasher = Sha256Hasher;

        for body in ["1件目", "2件目", "3件目"] {
            CaptureMemo::new(&db, &clock, &ids, &hasher)
                .execute(CaptureMemoInput {
                    workspace_id: "ws".into(),
                    workspace_name: "minos".into(),
                    body: body.into(),
                    document_id: None,
                    metas: Vec::new(),
                    context: None,
                    images: Vec::new(),
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
