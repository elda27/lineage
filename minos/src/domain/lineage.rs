//! Lineage（変換履歴）と、その真正性を担保する hash-chain。
//!
//! docs/concept/MINIMAL_ARCHITECTURE.md「4. Lineage の真正性担保」に対応する。
//!
//! - Lineage は append-only（更新・削除しない）
//! - 各 link は workspace 内の連番 `seq` を持つ
//! - `content_hash = SHA-256(正規化(seq, source, target, relation_type, actor, created_at, prev_hash))`
//! - `prev_hash` は直前 link の `content_hash`（先頭は genesis 定数）

use std::collections::BTreeMap;

use serde_json::Value;

use crate::domain::shared::{Hasher, canonicalize};

/// 鎖の先頭に使う定数。
pub const GENESIS_HASH: &str =
    "0000000000000000000000000000000000000000000000000000000000000000";

/// relation_type の初期セット。
///
/// minos が使うのは `DERIVED_FROM` だけだが、fullos・クラウド側と同じ語彙を1か所で持つ。
#[allow(dead_code)]
pub mod relation {
    /// 行・セルに対するメモ。
    pub const MEMO_FOR: &str = "memo_for";
    /// 添付。
    pub const ATTACHMENT_FOR: &str = "attachment_for";
    /// 参照。
    pub const REFERENCES: &str = "references";
    /// 何かから派生して作られた（minos のクイック入力はこれ）。
    pub const DERIVED_FROM: &str = "derived_from";
}

/// 鎖に追記する内容（seq / prev_hash / content_hash はまだ決まっていない）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineageInput {
    pub workspace_id: String,
    pub source_kind: String,
    pub source_id: String,
    pub target_kind: String,
    pub target_id: String,
    pub relation_type: String,
    pub actor: String,
    pub created_at: String,
}

/// 台帳に確定した1件。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineageRecord {
    pub id: String,
    pub workspace_id: String,
    pub seq: i64,
    pub source_kind: String,
    pub source_id: String,
    pub target_kind: String,
    pub target_id: String,
    pub relation_type: String,
    pub actor: String,
    pub created_at: String,
    pub content_hash: String,
    pub prev_hash: String,
}

impl LineageRecord {
    /// ハッシュ計算の対象になるフィールドだけを取り出す。
    ///
    /// `id` は含めない。ID は保存の都合で決まる値であり、
    /// 「何から何が作られたか」という事実の一部ではないため。
    fn canonical_fields(&self, prev_hash: &str) -> BTreeMap<&'static str, Value> {
        let mut fields = BTreeMap::new();
        fields.insert("actor", Value::from(self.actor.clone()));
        fields.insert("created_at", Value::from(self.created_at.clone()));
        fields.insert("prev_hash", Value::from(prev_hash));
        fields.insert("relation_type", Value::from(self.relation_type.clone()));
        fields.insert("seq", Value::from(self.seq));
        fields.insert("source_id", Value::from(self.source_id.clone()));
        fields.insert("source_kind", Value::from(self.source_kind.clone()));
        fields.insert("target_id", Value::from(self.target_id.clone()));
        fields.insert("target_kind", Value::from(self.target_kind.clone()));
        fields.insert("workspace_id", Value::from(self.workspace_id.clone()));
        fields
    }
}

/// 検証結果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VerifyResult {
    Ok { checked: usize },
    /// `seq` の位置で鎖が壊れている。
    Broken { broken_at: i64, reason: BrokenReason },
}

impl VerifyResult {
    #[allow(dead_code)]
    pub fn is_ok(&self) -> bool {
        matches!(self, VerifyResult::Ok { .. })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BrokenReason {
    /// prev_hash が直前レコードの content_hash と一致しない。
    PrevHashMismatch,
    /// 記録された content_hash が再計算値と一致しない（＝改ざん）。
    ContentHashMismatch,
    /// seq が 1 から始まる連番になっていない。
    SequenceGap,
}

/// hash-chain を伸ばす・検証するドメインサービス。
pub struct LineageLedger<'a> {
    hasher: &'a dyn Hasher,
}

impl<'a> LineageLedger<'a> {
    pub fn new(hasher: &'a dyn Hasher) -> Self {
        Self { hasher }
    }

    /// 直前レコードを受け取り、鎖を1つ伸ばす。
    pub fn append_next(
        &self,
        prev: Option<&LineageRecord>,
        id: String,
        input: LineageInput,
    ) -> LineageRecord {
        let seq = prev.map(|p| p.seq).unwrap_or(0) + 1;
        let prev_hash = prev
            .map(|p| p.content_hash.clone())
            .unwrap_or_else(|| GENESIS_HASH.to_string());

        let mut record = LineageRecord {
            id,
            workspace_id: input.workspace_id,
            seq,
            source_kind: input.source_kind,
            source_id: input.source_id,
            target_kind: input.target_kind,
            target_id: input.target_id,
            relation_type: input.relation_type,
            actor: input.actor,
            created_at: input.created_at,
            content_hash: String::new(),
            prev_hash: prev_hash.clone(),
        };
        record.content_hash = self
            .hasher
            .sha256_hex(&canonicalize(&record.canonical_fields(&prev_hash)));
        record
    }

    /// 台帳全体を再計算して鎖の整合性を検証する（`records` は seq 昇順）。
    pub fn verify(&self, records: &[LineageRecord]) -> VerifyResult {
        let mut prev_hash = GENESIS_HASH.to_string();

        for (index, record) in records.iter().enumerate() {
            let expected_seq = index as i64 + 1;
            if record.seq != expected_seq {
                return VerifyResult::Broken {
                    broken_at: record.seq,
                    reason: BrokenReason::SequenceGap,
                };
            }
            if record.prev_hash != prev_hash {
                return VerifyResult::Broken {
                    broken_at: record.seq,
                    reason: BrokenReason::PrevHashMismatch,
                };
            }

            let expected = self
                .hasher
                .sha256_hex(&canonicalize(&record.canonical_fields(&prev_hash)));
            if record.content_hash != expected {
                return VerifyResult::Broken {
                    broken_at: record.seq,
                    reason: BrokenReason::ContentHashMismatch,
                };
            }

            prev_hash = record.content_hash.clone();
        }

        VerifyResult::Ok {
            checked: records.len(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// domain のテストが infrastructure に依存しないよう、ここで閉じたハッシュ実装を使う。
    struct TestHasher;
    impl Hasher for TestHasher {
        fn sha256_hex(&self, input: &str) -> String {
            use sha2::{Digest, Sha256};
            format!("{:x}", Sha256::digest(input.as_bytes()))
        }
    }

    fn input(target: &str) -> LineageInput {
        LineageInput {
            workspace_id: "ws".into(),
            source_kind: "app".into(),
            source_id: "chrome.exe".into(),
            target_kind: "document".into(),
            target_id: target.into(),
            relation_type: relation::DERIVED_FROM.into(),
            actor: "local".into(),
            created_at: "2026-08-08T00:00:00Z".into(),
        }
    }

    fn chain(len: usize) -> Vec<LineageRecord> {
        let hasher = TestHasher;
        let ledger = LineageLedger::new(&hasher);
        let mut records: Vec<LineageRecord> = Vec::new();
        for i in 0..len {
            let next = ledger.append_next(records.last(), format!("link-{i}"), input(&format!("doc-{i}")));
            records.push(next);
        }
        records
    }

    #[test]
    fn first_link_starts_from_genesis() {
        let records = chain(1);
        assert_eq!(records[0].seq, 1);
        assert_eq!(records[0].prev_hash, GENESIS_HASH);
        assert_eq!(records[0].content_hash.len(), 64);
    }

    #[test]
    fn chain_links_to_previous_content_hash() {
        let records = chain(3);
        assert_eq!(records[1].prev_hash, records[0].content_hash);
        assert_eq!(records[2].prev_hash, records[1].content_hash);
    }

    #[test]
    fn verify_accepts_an_untouched_chain() {
        let hasher = TestHasher;
        let ledger = LineageLedger::new(&hasher);
        assert_eq!(ledger.verify(&chain(5)), VerifyResult::Ok { checked: 5 });
    }

    #[test]
    fn verify_detects_a_tampered_record() {
        let hasher = TestHasher;
        let ledger = LineageLedger::new(&hasher);
        let mut records = chain(5);
        records[2].source_id = "notepad.exe".into();

        assert_eq!(
            ledger.verify(&records),
            VerifyResult::Broken {
                broken_at: 3,
                reason: BrokenReason::ContentHashMismatch,
            }
        );
    }

    #[test]
    fn verify_detects_a_removed_record() {
        let hasher = TestHasher;
        let ledger = LineageLedger::new(&hasher);
        let mut records = chain(5);
        records.remove(2);

        // 3件目以降の seq がずれるため、連番の破れとして検出される。
        assert_eq!(
            ledger.verify(&records),
            VerifyResult::Broken {
                broken_at: 4,
                reason: BrokenReason::SequenceGap,
            }
        );
    }

    #[test]
    fn verify_accepts_an_empty_ledger() {
        let hasher = TestHasher;
        let ledger = LineageLedger::new(&hasher);
        assert_eq!(ledger.verify(&[]), VerifyResult::Ok { checked: 0 });
    }
}
