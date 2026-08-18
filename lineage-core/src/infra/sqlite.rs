//! SQLite による port の実装。
//!
//! スキーマは `db/schema.sql` 1本（ローカル SQLite とクラウド D1 で共通）。
//! ここに閉じ込めるのは SQL だけで、鎖の作り方（hash-chain）は domain 側にある。

use std::cell::RefCell;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use rusqlite::{Connection, OptionalExtension, Row, params};

use crate::domain::automation::{
    AutomationRule, AutomationRun, BackendConfig, BackendKind, MemoSnapshot, RunStatus, Trigger,
    TriggerKind,
};
use crate::domain::capture::{DOCUMENT_TYPE_MEMO, DocumentAsset};
use crate::domain::lineage::LineageRecord;
use crate::domain::meta::{DocumentMetadata, MetaAssignment, MetaSource, MetaTag};
use crate::domain::ports::{
    AutomationRuleQuery, AutomationRunStore, AutomationStore, AutomationTx, CaptureStore,
    CaptureTx, LedgerTx, LineageQuery, MemoQuery, MetaTagQuery, SettingsRepository, TagRepository,
};
use crate::domain::tag::{AutomationBinding, TagDefinition, TagKind, ViewBinding};

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
            std::fs::create_dir_all(parent).with_context(|| {
                format!("データディレクトリを作成できません: {}", parent.display())
            })?;
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

    #[cfg(any(test, feature = "testing"))]
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

/// document と link の SQL。記録の取り込みと自動化の結果保存で同じものを使う。
///
/// hash-chain の書き込みが2か所に分かれると、片方だけ直したときに鎖の作り方が
/// ずれる。SQL は1本に保ち、トランザクションの型だけを分ける。
mod ledger_sql {
    use super::*;

    pub fn insert_document(tx: &rusqlite::Transaction<'_>, document: &DocumentAsset) -> Result<()> {
        tx.execute(
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

    pub fn last_link(
        tx: &rusqlite::Transaction<'_>,
        workspace_id: &str,
    ) -> Result<Option<LineageRecord>> {
        let record = tx
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

    pub fn append_link(tx: &rusqlite::Transaction<'_>, link: &LineageRecord) -> Result<()> {
        tx.execute(
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

impl LedgerTx for SqliteCaptureTx<'_> {
    fn insert_document(&mut self, document: &DocumentAsset) -> Result<()> {
        ledger_sql::insert_document(self.tx, document)
    }

    fn last_link(&mut self, workspace_id: &str) -> Result<Option<LineageRecord>> {
        ledger_sql::last_link(self.tx, workspace_id)
    }

    fn append_link(&mut self, link: &LineageRecord) -> Result<()> {
        ledger_sql::append_link(self.tx, link)
    }
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
            params![
                id,
                document_id,
                meta.label,
                meta.value,
                meta.source.as_str(),
                now
            ],
        )?;
        self.tx.execute(
            "INSERT OR IGNORE INTO tag_assignments(id,document_id,tag_id,value,source,created_at)
             SELECT ?1,?2,id,?3,?4,?5 FROM tag_definitions
             WHERE display_name=?6 AND deleted_at IS NULL ORDER BY managed DESC LIMIT 1",
            params![
                id,
                document_id,
                meta.value,
                meta.source.as_str(),
                now,
                meta.label
            ],
        )?;
        Ok(())
    }

    fn insert_document_metadata(
        &mut self,
        id: &str,
        document_id: &str,
        metadata: &DocumentMetadata,
        now: &str,
    ) -> Result<()> {
        self.tx.execute(
            "INSERT INTO document_metadata (id, document_id, key, value, source, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(document_id, key) DO UPDATE SET value=excluded.value, source=excluded.source",
            params![id, document_id, metadata.key, metadata.value, metadata.source, now],
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
        self.tx.execute(
            "INSERT INTO tag_definitions(id,workspace_id,kind,display_name,shorthand,enabled,managed,usage_count,last_used_at,deleted_at,created_at,updated_at)
             VALUES(?1,?2,'user',?3,NULL,1,0,1,?4,NULL,?4,?4)
             ON CONFLICT(workspace_id,display_name) DO UPDATE SET usage_count=usage_count+1,last_used_at=excluded.last_used_at,updated_at=excluded.updated_at",
            params![id, workspace_id, label, now],
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
            "SELECT id, workspace_id, display_name, shorthand, usage_count, last_used_at
             FROM tag_definitions WHERE (workspace_id = ?1 OR workspace_id='local')
               AND kind != 'metadata' AND enabled=1 AND deleted_at IS NULL
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

impl TagRepository for Database {
    fn list(&self, workspace_id: &str, include_deleted: bool) -> Result<Vec<TagDefinition>> {
        let conn = self.conn.borrow();
        let mut stmt = conn.prepare(
            "SELECT t.id,t.workspace_id,t.kind,t.display_name,t.shorthand,t.usage_count,t.last_used_at,
                    t.enabled,t.managed,t.deleted_at,v.view_id,a.recipe_name,a.ownership,a.enabled
             FROM tag_definitions t LEFT JOIN view_bindings v ON v.tag_id=t.id
             LEFT JOIN automation_bindings a ON a.tag_id=t.id
             WHERE (t.workspace_id=?1 OR t.workspace_id='local') AND (?2 OR t.deleted_at IS NULL)
             ORDER BY t.usage_count DESC,t.display_name")?;
        Ok(stmt
            .query_map(
                params![workspace_id, include_deleted],
                row_to_tag_definition,
            )?
            .collect::<rusqlite::Result<Vec<_>>>()?)
    }

    fn get(&self, id: &str) -> Result<Option<TagDefinition>> {
        let conn = self.conn.borrow();
        Ok(conn.query_row(
            "SELECT t.id,t.workspace_id,t.kind,t.display_name,t.shorthand,t.usage_count,t.last_used_at,
                    t.enabled,t.managed,t.deleted_at,v.view_id,a.recipe_name,a.ownership,a.enabled
             FROM tag_definitions t LEFT JOIN view_bindings v ON v.tag_id=t.id
             LEFT JOIN automation_bindings a ON a.tag_id=t.id WHERE t.id=?1",
            params![id], row_to_tag_definition).optional()?)
    }

    fn rename(&self, id: &str, name: &str, shorthand: Option<&str>, now: &str) -> Result<()> {
        self.conn.borrow().execute("UPDATE tag_definitions SET display_name=?2,shorthand=?3,updated_at=?4 WHERE id=?1 AND kind='user'", params![id,name,shorthand,now])?;
        Ok(())
    }
    fn soft_delete(&self, id: &str, now: &str) -> Result<()> {
        self.conn.borrow().execute("UPDATE tag_definitions SET deleted_at=?2,enabled=0,updated_at=?2 WHERE id=?1 AND kind='user'", params![id,now])?;
        Ok(())
    }
    fn set_enabled(&self, id: &str, enabled: bool, now: &str) -> Result<()> {
        self.conn.borrow().execute(
            "UPDATE tag_definitions SET enabled=?2,updated_at=?3 WHERE id=?1",
            params![id, enabled, now],
        )?;
        Ok(())
    }
    fn set_view_binding(
        &self,
        binding: Option<&ViewBinding>,
        tag_id: &str,
        now: &str,
    ) -> Result<()> {
        let conn = self.conn.borrow();
        if let Some(b) = binding {
            conn.execute("INSERT INTO view_bindings(tag_id,view_id,updated_at) VALUES(?1,?2,?3) ON CONFLICT(tag_id) DO UPDATE SET view_id=excluded.view_id,updated_at=excluded.updated_at",params![tag_id,b.view_id,now])?;
        } else {
            conn.execute("DELETE FROM view_bindings WHERE tag_id=?1", params![tag_id])?;
        }
        Ok(())
    }
    fn set_automation_binding(
        &self,
        binding: Option<&AutomationBinding>,
        tag_id: &str,
        now: &str,
    ) -> Result<()> {
        let conn = self.conn.borrow();
        if let Some(b) = binding {
            let ownership = if b.managed { "managed" } else { "external" };
            conn.execute("INSERT INTO automation_bindings(tag_id,recipe_name,ownership,enabled,updated_at) VALUES(?1,?2,?3,?4,?5) ON CONFLICT(tag_id) DO UPDATE SET recipe_name=excluded.recipe_name,ownership=excluded.ownership,enabled=excluded.enabled,updated_at=excluded.updated_at",params![tag_id,b.recipe_name,ownership,b.enabled,now])?;
        } else {
            conn.execute(
                "DELETE FROM automation_bindings WHERE tag_id=?1",
                params![tag_id],
            )?;
        }
        Ok(())
    }
}

fn row_to_tag_definition(row: &Row<'_>) -> rusqlite::Result<TagDefinition> {
    let tag_id: String = row.get(0)?;
    let view: Option<String> = row.get(10)?;
    let recipe: Option<String> = row.get(11)?;
    Ok(TagDefinition {
        id: tag_id.clone(),
        workspace_id: row.get(1)?,
        kind: TagKind::parse(&row.get::<_, String>(2)?),
        display_name: row.get(3)?,
        shorthand: row.get(4)?,
        usage_count: row.get(5)?,
        last_used_at: row.get(6)?,
        enabled: row.get(7)?,
        managed: row.get(8)?,
        deleted_at: row.get(9)?,
        view: view.map(|view_id| ViewBinding {
            tag_id: tag_id.clone(),
            view_id,
        }),
        automation: recipe.map(|recipe_name| AutomationBinding {
            tag_id,
            recipe_name,
            managed: row.get::<_, String>(12).unwrap_or_default() == "managed",
            enabled: row.get(13).unwrap_or(true),
        }),
    })
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

impl AutomationStore for Database {
    fn transact(&self, work: &mut dyn FnMut(&mut dyn AutomationTx) -> Result<()>) -> Result<()> {
        let mut conn = self.conn.borrow_mut();
        let tx = conn.transaction()?;
        {
            let mut automation_tx = SqliteAutomationTx { tx: &tx };
            work(&mut automation_tx)?;
        }
        tx.commit()?;
        Ok(())
    }
}

struct SqliteAutomationTx<'a> {
    tx: &'a rusqlite::Transaction<'a>,
}

impl LedgerTx for SqliteAutomationTx<'_> {
    fn insert_document(&mut self, document: &DocumentAsset) -> Result<()> {
        ledger_sql::insert_document(self.tx, document)
    }

    fn last_link(&mut self, workspace_id: &str) -> Result<Option<LineageRecord>> {
        ledger_sql::last_link(self.tx, workspace_id)
    }

    fn append_link(&mut self, link: &LineageRecord) -> Result<()> {
        ledger_sql::append_link(self.tx, link)
    }
}

impl AutomationTx for SqliteAutomationTx<'_> {
    fn finish_run(&mut self, run: &AutomationRun) -> Result<()> {
        self.tx.execute(
            "UPDATE automation_runs
                SET status = ?2, result_document_id = ?3, error = ?4, finished_at = ?5
              WHERE id = ?1",
            params![
                run.id,
                run.status.as_str(),
                run.result_document_id,
                run.error,
                run.finished_at,
            ],
        )?;
        Ok(())
    }
}

impl AutomationRuleQuery for Database {
    fn all(&self, workspace_id: &str) -> Result<Vec<AutomationRule>> {
        let conn = self.conn.borrow();
        let mut statement = conn.prepare(
            "SELECT id, workspace_id, name, description, prompt, backend_kind, backend_config,
                    trigger_kind, trigger_config, enabled, created_at, updated_at
             FROM automation_rules WHERE workspace_id = ?1 ORDER BY created_at ASC",
        )?;
        let rows = statement
            .query_map(params![workspace_id], row_to_rule)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        rows.into_iter().collect()
    }

    fn get(&self, id: &str) -> Result<Option<AutomationRule>> {
        let conn = self.conn.borrow();
        let row = conn
            .query_row(
                "SELECT id, workspace_id, name, description, prompt, backend_kind, backend_config,
                        trigger_kind, trigger_config, enabled, created_at, updated_at
                 FROM automation_rules WHERE id = ?1",
                params![id],
                row_to_rule,
            )
            .optional()?;
        row.transpose()
    }
}

impl AutomationRunStore for Database {
    fn start(&self, run: &AutomationRun) -> Result<()> {
        let conn = self.conn.borrow();
        conn.execute(
            "INSERT INTO automation_runs
                 (id, workspace_id, rule_id, source_document_id, result_document_id,
                  status, backend_kind, error, started_at, finished_at)
             VALUES (?1, ?2, ?3, ?4, NULL, ?5, ?6, NULL, ?7, NULL)",
            params![
                run.id,
                run.workspace_id,
                run.rule_id,
                run.source_document_id,
                run.status.as_str(),
                run.backend.as_str(),
                run.started_at,
            ],
        )?;
        Ok(())
    }

    fn recent(&self, workspace_id: &str, limit: usize) -> Result<Vec<AutomationRun>> {
        let conn = self.conn.borrow();
        let mut statement = conn.prepare(
            "SELECT id, workspace_id, rule_id, source_document_id, result_document_id,
                    status, backend_kind, error, started_at, finished_at
             FROM automation_runs WHERE workspace_id = ?1
             ORDER BY started_at DESC LIMIT ?2",
        )?;
        let rows = statement
            .query_map(params![workspace_id, limit as i64], row_to_run)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        rows.into_iter().collect()
    }

    fn unprocessed_memos(
        &self,
        workspace_id: &str,
        rule_id: &str,
        scan_limit: usize,
    ) -> Result<Vec<MemoSnapshot>> {
        let conn = self.conn.borrow();
        // 「成功済み or 実行中」の run があるものを除く。失敗した記録は残るので、
        // 鍵の未登録や通信断のような一時的な失敗は次の poll で自然に再試行される。
        //
        // ゴミ箱に入れた記録（document_states.deleted_at）も除く。利用者が捨てたものに
        // 自動化を当て続けると、結果の document だけが増えていく。
        let mut statement = conn.prepare(
            "SELECT id, title, body_text, created_at
             FROM documents d
             WHERE d.workspace_id = ?1
               AND d.document_type = ?2
               AND NOT EXISTS (
                     SELECT 1 FROM automation_runs r
                     WHERE r.rule_id = ?3
                       AND r.source_document_id = d.id
                       AND r.status IN ('running', 'succeeded')
                   )
               AND NOT EXISTS (
                     SELECT 1 FROM document_states s
                     WHERE s.document_id = d.id
                       AND s.deleted_at IS NOT NULL
                   )
             ORDER BY d.created_at DESC LIMIT ?4",
        )?;
        let bases = statement
            .query_map(
                params![workspace_id, DOCUMENT_TYPE_MEMO, rule_id, scan_limit as i64],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, Option<String>>(2)?,
                        row.get::<_, String>(3)?,
                    ))
                },
            )?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        drop(statement);
        bases
            .into_iter()
            .map(|(id, title, body_text, created_at)| {
                Ok(MemoSnapshot {
                    metas: metas_of(&conn, &id)?,
                    id,
                    title,
                    body_text: body_text.unwrap_or_default(),
                    created_at,
                })
            })
            .collect()
    }

    fn last_started_at(&self, rule_id: &str) -> Result<Option<String>> {
        let conn = self.conn.borrow();
        let started_at = conn
            .query_row(
                "SELECT max(started_at) FROM automation_runs WHERE rule_id = ?1",
                params![rule_id],
                |row| row.get::<_, Option<String>>(0),
            )
            .optional()?
            .flatten();
        Ok(started_at)
    }
}

impl MemoQuery for Database {
    fn get(&self, workspace_id: &str, document_id: &str) -> Result<Option<MemoSnapshot>> {
        let conn = self.conn.borrow();
        let base = conn
            .query_row(
                "SELECT id, title, body_text, created_at FROM documents
                 WHERE workspace_id = ?1 AND id = ?2 AND document_type = ?3",
                params![workspace_id, document_id, DOCUMENT_TYPE_MEMO],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, Option<String>>(2)?,
                        row.get::<_, String>(3)?,
                    ))
                },
            )
            .optional()?;

        let Some((id, title, body_text, created_at)) = base else {
            return Ok(None);
        };
        Ok(Some(MemoSnapshot {
            metas: metas_of(&conn, &id)?,
            id,
            title,
            body_text: body_text.unwrap_or_default(),
            created_at,
        }))
    }
}

/// 記録に付いたメタ情報を読み出す。
fn metas_of(conn: &Connection, document_id: &str) -> Result<Vec<MetaAssignment>> {
    let mut statement = conn.prepare(
        "SELECT label, value, source FROM document_meta WHERE document_id = ?1 ORDER BY label",
    )?;
    let metas = statement
        .query_map(params![document_id], |row| {
            Ok(MetaAssignment {
                label: row.get(0)?,
                value: row.get(1)?,
                source: MetaSource::parse(&row.get::<_, String>(2)?),
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(metas)
}

/// `backend_kind` / `trigger_kind` / JSON 列をドメインの型へ戻す。
///
/// 行の読み出し自体は成功したが中身が壊れている（未知の種別・壊れた JSON）ことが
/// ありうるので、`rusqlite::Result<Result<_>>` の二段で返して呼び出し側で潰す。
fn row_to_rule(row: &Row<'_>) -> rusqlite::Result<Result<AutomationRule>> {
    let id: String = row.get(0)?;
    let backend_kind: String = row.get(5)?;
    let backend_config: String = row.get(6)?;
    let trigger_kind: String = row.get(7)?;
    let trigger_config: String = row.get(8)?;

    Ok((|| {
        let backend = BackendKind::parse(&backend_kind)
            .with_context(|| format!("ルール {id} の backend_kind が不正です: {backend_kind}"))?;
        let trigger_kind_parsed = TriggerKind::parse(&trigger_kind)
            .with_context(|| format!("ルール {id} の trigger_kind が不正です: {trigger_kind}"))?;
        let config: BackendConfig = serde_json::from_str(&backend_config)
            .with_context(|| format!("ルール {id} の backend_config を解釈できません"))?;
        let trigger: Trigger = serde_json::from_str(&trigger_config)
            .with_context(|| format!("ルール {id} の trigger_config を解釈できません"))?;

        Ok(AutomationRule {
            id: id.clone(),
            workspace_id: row_string(row, 1),
            name: row_string(row, 2),
            description: row_opt_string(row, 3),
            prompt: row_string(row, 4),
            backend,
            backend_config: config,
            trigger_kind: trigger_kind_parsed,
            trigger,
            enabled: row.get::<_, i64>(9).unwrap_or(1) != 0,
            created_at: row_string(row, 10),
            updated_at: row_string(row, 11),
        })
    })())
}

fn row_to_run(row: &Row<'_>) -> rusqlite::Result<Result<AutomationRun>> {
    let id: String = row.get(0)?;
    let status: String = row.get(5)?;
    let backend_kind: String = row.get(6)?;

    Ok((|| {
        let status = RunStatus::parse(&status)
            .with_context(|| format!("実行 {id} の status が不正です: {status}"))?;
        let backend = BackendKind::parse(&backend_kind)
            .with_context(|| format!("実行 {id} の backend_kind が不正です: {backend_kind}"))?;

        Ok(AutomationRun {
            id: id.clone(),
            workspace_id: row_string(row, 1),
            rule_id: row_string(row, 2),
            source_document_id: row_string(row, 3),
            result_document_id: row_opt_string(row, 4),
            status,
            backend,
            error: row_opt_string(row, 7),
            started_at: row_string(row, 8),
            finished_at: row_opt_string(row, 9),
        })
    })())
}

/// 列は上の SELECT で必ず取れるので、取り出せない事態は起こらない。
fn row_string(row: &Row<'_>, index: usize) -> String {
    row.get(index).unwrap_or_default()
}

fn row_opt_string(row: &Row<'_>, index: usize) -> Option<String> {
    row.get(index).unwrap_or_default()
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

#[cfg(any(test, feature = "testing"))]
impl Database {
    /// 生の接続を貸す。テストで前提データを直接書き込むためだけに使う。
    pub fn connection_for_test(&self) -> std::cell::Ref<'_, Connection> {
        self.conn.borrow()
    }

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
    use crate::infra::crypto::Sha256Hasher;

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
                 ('workspaces', 'documents', 'links', 'meta_tags', 'document_meta',
                  'document_states')",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 6);
    }

    #[test]
    fn round_trips_the_lineage_chain() {
        let db = Database::open_in_memory().unwrap();
        let hasher = Sha256Hasher;
        let ledger = LineageLedger::new(&hasher);

        CaptureStore::transact(&db, &mut |tx: &mut dyn CaptureTx| {
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

        let result = CaptureStore::transact(&db, &mut |tx: &mut dyn CaptureTx| {
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

        let result = CaptureStore::transact(&db, &mut |tx: &mut dyn CaptureTx| {
            let first = link(&ledger, None, "a");
            tx.append_link(&first)?;
            // 同じ seq をもう一度追記しようとする（＝鎖の分岐）。
            tx.append_link(&link(&ledger, None, "b"))?;
            Ok(())
        });

        assert!(result.is_err());
    }

    #[test]
    fn rename_keeps_stable_id_and_soft_delete_keeps_assignments() {
        let db = Database::open_in_memory().unwrap();
        db.conn.borrow().execute(
            "INSERT INTO tag_definitions VALUES('tag-1','ws','user','old',NULL,1,0,1,NULL,NULL,'now','now')", [],
        ).unwrap();
        db.conn
            .borrow()
            .execute(
                "INSERT INTO tag_assignments VALUES('a-1','doc-1','tag-1',NULL,'user','now')",
                [],
            )
            .unwrap();

        TagRepository::rename(&db, "tag-1", "new", Some("n"), "later").unwrap();
        let renamed = TagRepository::get(&db, "tag-1").unwrap().unwrap();
        assert_eq!(renamed.id, "tag-1");
        assert_eq!(renamed.display_name, "new");

        TagRepository::soft_delete(&db, "tag-1", "deleted").unwrap();
        assert!(TagRepository::list(&db, "ws", false).unwrap().is_empty());
        let provenance: String = db
            .conn
            .borrow()
            .query_row(
                "SELECT tag_id FROM tag_assignments WHERE id='a-1'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(provenance, "tag-1");
    }
}
