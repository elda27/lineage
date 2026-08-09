//! SQLite による port の実装。
//!
//! スキーマは `db/schema.sql` 1本（ローカル SQLite とクラウド D1 で共通）。
//! ここに閉じ込めるのは SQL だけで、鎖の作り方（hash-chain）は domain 側にある。

use std::cell::RefCell;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use rusqlite::{Connection, OptionalExtension, Row, params};

use crate::domain::capture::DocumentAsset;
use crate::domain::lineage::LineageRecord;
use crate::domain::meta::{MetaAssignment, MetaTag};
use crate::domain::ports::{
    CaptureStore, CaptureTx, LineageQuery, MetaTagQuery, SettingsRepository,
};

/// ローカルとクラウドで共通のスキーマ。
const SCHEMA_SQL: &str = include_str!("../../../db/schema.sql");

/// ローカル DB のファイル名。
const DATABASE_FILE_NAME: &str = "lineage.db";

/// 接続を1本だけ持つローカルストア。
///
/// minos は単一利用者・単一プロセスなので接続は1本で足りる。
/// gpui のメインスレッドから同期的に呼ぶ前提のため `RefCell` で足りる。
pub struct Database {
    conn: RefCell<Connection>,
}

impl Database {
    /// 既定のデータディレクトリ（`%LOCALAPPDATA%\minos`）の DB を開く。
    pub fn open_default() -> Result<Self> {
        let path = Self::default_path()?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("データディレクトリを作成できません: {}", parent.display()))?;
        }
        Self::open(&path)
    }

    pub fn default_path() -> Result<PathBuf> {
        let dir = dirs::data_local_dir()
            .context("ローカルアプリケーションデータのディレクトリを特定できません")?;
        Ok(dir.join("minos").join(DATABASE_FILE_NAME))
    }

    pub fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open(path)
            .with_context(|| format!("DB を開けません: {}", path.display()))?;
        Self::from_connection(conn)
    }

    #[cfg(test)]
    pub fn open_in_memory() -> Result<Self> {
        Self::from_connection(Connection::open_in_memory()?)
    }

    fn from_connection(conn: Connection) -> Result<Self> {
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;
             PRAGMA busy_timeout = 3000;",
        )?;
        conn.execute_batch(SCHEMA_SQL)
            .context("スキーマの適用に失敗しました")?;
        Ok(Self {
            conn: RefCell::new(conn),
        })
    }
}

impl CaptureStore for Database {
    fn transact(&self, work: &mut dyn FnMut(&mut dyn CaptureTx) -> Result<()>) -> Result<()> {
        let mut conn = self.conn.borrow_mut();
        let tx = conn.transaction()?;
        {
            let mut capture_tx = SqliteCaptureTx { tx: &tx };
            work(&mut capture_tx)?;
        }
        tx.commit()?;
        Ok(())
    }
}

struct SqliteCaptureTx<'a> {
    tx: &'a rusqlite::Transaction<'a>,
}

impl CaptureTx for SqliteCaptureTx<'_> {
    fn ensure_workspace(&mut self, id: &str, name: &str, now: &str) -> Result<()> {
        self.tx.execute(
            "INSERT OR IGNORE INTO workspaces (id, name, owner_user_id, created_at)
             VALUES (?1, ?2, NULL, ?3)",
            params![id, name, now],
        )?;
        Ok(())
    }

    fn insert_document(&mut self, document: &DocumentAsset) -> Result<()> {
        self.tx.execute(
            "INSERT INTO documents
                 (id, workspace_id, title, body_text, blob_uri, document_type, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, NULL, ?5, ?6, ?7)",
            params![
                document.id,
                document.workspace_id,
                document.title,
                document.body_text,
                document.document_type,
                document.created_at,
                document.updated_at,
            ],
        )?;
        Ok(())
    }

    fn insert_document_meta(
        &mut self,
        id: &str,
        document_id: &str,
        meta: &MetaAssignment,
        now: &str,
    ) -> Result<()> {
        self.tx.execute(
            "INSERT INTO document_meta (id, document_id, label, value, source, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(document_id, label)
             DO UPDATE SET value = excluded.value, source = excluded.source",
            params![id, document_id, meta.label, meta.value, meta.source.as_str(), now],
        )?;
        Ok(())
    }

    fn learn_meta_tag(
        &mut self,
        id: &str,
        workspace_id: &str,
        label: &str,
        now: &str,
    ) -> Result<()> {
        self.tx.execute(
            "INSERT INTO meta_tags
                 (id, workspace_id, label, shorthand, usage_count, last_used_at, created_at)
             VALUES (?1, ?2, ?3, NULL, 1, ?4, ?4)
             ON CONFLICT(workspace_id, label)
             DO UPDATE SET usage_count = usage_count + 1, last_used_at = excluded.last_used_at",
            params![id, workspace_id, label, now],
        )?;
        Ok(())
    }

    fn last_link(&mut self, workspace_id: &str) -> Result<Option<LineageRecord>> {
        let record = self
            .tx
            .query_row(
                "SELECT id, workspace_id, seq, source_kind, source_id, target_kind, target_id,
                        relation_type, actor, created_at, content_hash, prev_hash
                 FROM links WHERE workspace_id = ?1 ORDER BY seq DESC LIMIT 1",
                params![workspace_id],
                row_to_lineage_record,
            )
            .optional()?;
        Ok(record)
    }

    fn append_link(&mut self, link: &LineageRecord) -> Result<()> {
        self.tx.execute(
            "INSERT INTO links
                 (id, workspace_id, seq, source_kind, source_id, target_kind, target_id,
                  relation_type, actor, created_at, content_hash, prev_hash)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                link.id,
                link.workspace_id,
                link.seq,
                link.source_kind,
                link.source_id,
                link.target_kind,
                link.target_id,
                link.relation_type,
                link.actor,
                link.created_at,
                link.content_hash,
                link.prev_hash,
            ],
        )?;
        Ok(())
    }
}

impl LineageQuery for Database {
    fn list(&self, workspace_id: &str) -> Result<Vec<LineageRecord>> {
        let conn = self.conn.borrow();
        let mut statement = conn.prepare(
            "SELECT id, workspace_id, seq, source_kind, source_id, target_kind, target_id,
                    relation_type, actor, created_at, content_hash, prev_hash
             FROM links WHERE workspace_id = ?1 ORDER BY seq ASC",
        )?;
        let records = statement
            .query_map(params![workspace_id], row_to_lineage_record)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(records)
    }
}

impl MetaTagQuery for Database {
    fn all(&self, workspace_id: &str, limit: usize) -> Result<Vec<MetaTag>> {
        let conn = self.conn.borrow();
        let mut statement = conn.prepare(
            "SELECT id, workspace_id, label, shorthand, usage_count, last_used_at
             FROM meta_tags WHERE workspace_id = ?1
             ORDER BY usage_count DESC, last_used_at DESC LIMIT ?2",
        )?;
        let tags = statement
            .query_map(params![workspace_id, limit as i64], |row| {
                Ok(MetaTag {
                    id: row.get(0)?,
                    workspace_id: row.get(1)?,
                    label: row.get(2)?,
                    shorthand: row.get(3)?,
                    usage_count: row.get(4)?,
                    last_used_at: row.get(5)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(tags)
    }
}

impl SettingsRepository for Database {
    fn all(&self, workspace_id: &str) -> Result<Vec<(String, String)>> {
        let conn = self.conn.borrow();
        let mut statement =
            conn.prepare("SELECT key, value FROM settings WHERE workspace_id = ?1")?;
        let entries = statement
            .query_map(params![workspace_id], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(entries)
    }

    fn set(&self, workspace_id: &str, key: &str, value: &str, now: &str) -> Result<()> {
        let conn = self.conn.borrow();
        conn.execute(
            "INSERT INTO settings (workspace_id, key, value, updated_at)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(workspace_id, key)
             DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
            params![workspace_id, key, value, now],
        )?;
        Ok(())
    }
}

fn row_to_lineage_record(row: &Row<'_>) -> rusqlite::Result<LineageRecord> {
    Ok(LineageRecord {
        id: row.get(0)?,
        workspace_id: row.get(1)?,
        seq: row.get(2)?,
        source_kind: row.get(3)?,
        source_id: row.get(4)?,
        target_kind: row.get(5)?,
        target_id: row.get(6)?,
        relation_type: row.get(7)?,
        actor: row.get(8)?,
        created_at: row.get(9)?,
        content_hash: row.get(10)?,
        prev_hash: row.get(11)?,
    })
}

#[cfg(test)]
impl Database {
    /// `(label, value, source)` を返す（テスト用）。
    pub fn metas_of_document(
        &self,
        document_id: &str,
    ) -> Result<Vec<(String, Option<String>, String)>> {
        let conn = self.conn.borrow();
        let mut statement = conn.prepare(
            "SELECT label, value, source FROM document_meta WHERE document_id = ?1 ORDER BY label",
        )?;
        let rows = statement
            .query_map(params![document_id], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    /// 台帳を直接書き換える。改ざん検知のテストのためだけに存在する。
    pub fn force_update_link_actor_for_test(
        &self,
        workspace_id: &str,
        seq: i64,
        actor: &str,
    ) -> Result<()> {
        let conn = self.conn.borrow();
        conn.execute(
            "UPDATE links SET actor = ?3 WHERE workspace_id = ?1 AND seq = ?2",
            params![workspace_id, seq, actor],
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::lineage::{LineageInput, LineageLedger};
    use crate::infrastructure::crypto::Sha256Hasher;

    fn link(ledger: &LineageLedger<'_>, prev: Option<&LineageRecord>, id: &str) -> LineageRecord {
        ledger.append_next(
            prev,
            id.to_string(),
            LineageInput {
                workspace_id: "ws".into(),
                source_kind: "minos".into(),
                source_id: "capture".into(),
                target_kind: "document".into(),
                target_id: format!("doc-{id}"),
                relation_type: "derived_from".into(),
                actor: "local".into(),
                created_at: "2026-08-08T00:00:00Z".into(),
            },
        )
    }

    #[test]
    fn applies_the_shared_schema_on_open() {
        let db = Database::open_in_memory().unwrap();
        let conn = db.conn.borrow();
        let count: i64 = conn
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE type = 'table' AND name IN
                 ('workspaces', 'documents', 'links', 'meta_tags', 'document_meta')",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 5);
    }

    #[test]
    fn round_trips_the_lineage_chain() {
        let db = Database::open_in_memory().unwrap();
        let hasher = Sha256Hasher;
        let ledger = LineageLedger::new(&hasher);

        db.transact(&mut |tx| {
            tx.ensure_workspace("ws", "minos", "2026-08-08T00:00:00Z")?;
            let first = link(&ledger, None, "a");
            tx.append_link(&first)?;
            let second = link(&ledger, Some(&first), "b");
            tx.append_link(&second)?;
            Ok(())
        })
        .unwrap();

        let records = db.list("ws").unwrap();
        assert_eq!(records.len(), 2);
        assert_eq!(records[0].seq, 1);
        assert_eq!(records[1].prev_hash, records[0].content_hash);
        assert!(ledger.verify(&records).is_ok());
    }

    #[test]
    fn a_failed_transaction_leaves_no_partial_write() {
        let db = Database::open_in_memory().unwrap();
        let hasher = Sha256Hasher;
        let ledger = LineageLedger::new(&hasher);

        let result = db.transact(&mut |tx| {
            tx.ensure_workspace("ws", "minos", "2026-08-08T00:00:00Z")?;
            tx.append_link(&link(&ledger, None, "a"))?;
            anyhow::bail!("途中で失敗");
        });

        assert!(result.is_err());
        assert!(db.list("ws").unwrap().is_empty());
    }

    #[test]
    fn duplicate_seq_is_rejected_by_the_schema() {
        let db = Database::open_in_memory().unwrap();
        let hasher = Sha256Hasher;
        let ledger = LineageLedger::new(&hasher);

        let result = db.transact(&mut |tx| {
            let first = link(&ledger, None, "a");
            tx.append_link(&first)?;
            // 同じ seq をもう一度追記しようとする（＝鎖の分岐）。
            tx.append_link(&link(&ledger, None, "b"))?;
            Ok(())
        });

        assert!(result.is_err());
    }
}
