//! SQLite による port の実装。
//!
//! スキーマは `db/schema.sql` 1本（ローカル SQLite とクラウド D1 で共通）。
//! ここに閉じ込めるのは SQL だけで、鎖の作り方（hash-chain）は domain 側にある。

use std::cell::RefCell;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, ensure};
use rusqlite::{Connection, OptionalExtension, Row, TransactionBehavior, params, params_from_iter};

use crate::domain::automation::{
    AutomationRule, AutomationRun, BackendConfig, BackendKind, MemoSnapshot, RunStatus, Trigger,
    TriggerKind,
};
use crate::domain::capture::{DOCUMENT_TYPE_MEMO, DocumentAsset};
use crate::domain::lineage::LineageRecord;
use crate::domain::meta::{DocumentMetadata, MetaAssignment, MetaSource, MetaTag};
use crate::domain::mutation::{
    MutationOperation, MutationRequest, MutationResult, MutationStatus, NullablePatch,
};
use crate::domain::ports::{
    AutomationRuleQuery, AutomationRunStore, AutomationStore, AutomationTx, CaptureStore,
    CaptureTx, LedgerTx, LineageQuery, MemoQuery, MetaTagQuery, MutationStore, SettingsRepository,
    TagRepository,
};
use crate::domain::tag::{AutomationBinding, TagDefinition, TagKind, ViewBinding};

/// ローカルとクラウドで共通のスキーマ。
const SCHEMA_SQL: &str = include_str!("../../../db/schema.sql");

/// ローカル DB のファイル名。
const DATABASE_FILE_NAME: &str = "lineage.db";

/// 接続を1本だけ持つローカルストア。
///
/// 各 minos / agentos プロセス内では接続は1本で足りる。プロセス間の競合は WAL、
/// busy timeout、immediate transaction で調停する。gpui のメインスレッドから同期的に
/// 呼ぶ前提のため、プロセス内の所有には `RefCell` を使う。
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

    fn from_connection(mut conn: Connection) -> Result<Self> {
        conn.execute_batch(
            "PRAGMA busy_timeout = 3000;
             PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;",
        )?;
        apply_schema(&mut conn, SCHEMA_SQL)?;
        Ok(Self {
            conn: RefCell::new(conn),
        })
    }
}

/// 互換更新と現行スキーマを同じトランザクションで適用する。
fn apply_schema(conn: &mut Connection, schema_sql: &str) -> Result<()> {
    let tx = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .context("スキーマ更新トランザクションを開始できません")?;
    upgrade_automation_runs(&tx).context("既存 automation_runs の更新に失敗しました")?;
    upgrade_local_mutations(&tx).context("既存 local_mutations の更新に失敗しました")?;
    tx.execute_batch(schema_sql)
        .context("スキーマの適用に失敗しました")?;
    tx.commit().context("スキーマ更新を確定できません")?;
    Ok(())
}

/// `automation_runs` に後から追加された列を、共有スキーマの適用前に補う。
///
/// `CREATE TABLE IF NOT EXISTS` は既存テーブルの列を更新しないため、旧DBでは
/// スキーマ末尾の `execution_key` インデックス作成時に起動が失敗していた。
/// 列ごとに存在を確認してから `ALTER TABLE` することで、旧DB・途中まで適用された
/// DB・新規DBのいずれでも安全に再実行できるようにする。ALTER TABLE は同じ
/// トランザクション内で行うため、途中で失敗した場合も一部だけ残らない。
fn upgrade_automation_runs(tx: &rusqlite::Transaction<'_>) -> Result<()> {
    let table_exists: bool = tx.query_row(
        "SELECT EXISTS(
             SELECT 1 FROM sqlite_master
             WHERE type = 'table' AND name = 'automation_runs'
         )",
        [],
        |row| row.get(0),
    )?;
    if !table_exists {
        return Ok(());
    }

    let existing_columns = {
        let mut statement = tx.prepare("PRAGMA table_info(automation_runs)")?;
        statement
            .query_map([], |row| row.get::<_, String>(1))?
            .collect::<rusqlite::Result<Vec<_>>>()?
    };

    // These columns were introduced after the original ten-column table. Keep each
    // definition identical to db/schema.sql so the resulting table is compatible
    // with both local SQLite and D1.
    const ADDED_COLUMNS: [(&str, &str); 8] = [
        (
            "tag_id",
            "ALTER TABLE automation_runs ADD COLUMN tag_id TEXT",
        ),
        (
            "recipe_name",
            "ALTER TABLE automation_runs ADD COLUMN recipe_name TEXT",
        ),
        (
            "recipe_ownership",
            "ALTER TABLE automation_runs ADD COLUMN recipe_ownership TEXT",
        ),
        (
            "processing_fingerprint",
            "ALTER TABLE automation_runs ADD COLUMN processing_fingerprint TEXT",
        ),
        (
            "input_fingerprint",
            "ALTER TABLE automation_runs ADD COLUMN input_fingerprint TEXT",
        ),
        (
            "execution_key",
            "ALTER TABLE automation_runs ADD COLUMN execution_key TEXT",
        ),
        (
            "output_fingerprint",
            "ALTER TABLE automation_runs ADD COLUMN output_fingerprint TEXT",
        ),
        (
            "forced",
            "ALTER TABLE automation_runs ADD COLUMN forced INTEGER NOT NULL DEFAULT 0",
        ),
    ];

    let missing = ADDED_COLUMNS
        .iter()
        .filter(|(name, _)| {
            !existing_columns
                .iter()
                .any(|column| column.eq_ignore_ascii_case(name))
        })
        .map(|(_, statement)| *statement)
        .collect::<Vec<_>>();
    if missing.is_empty() {
        return Ok(());
    }

    for statement in missing {
        tx.execute_batch(statement)?;
    }
    Ok(())
}

/// 差分 mutation API の初期版で作られた台帳へ receipt 列を補う。
///
/// `CREATE TABLE IF NOT EXISTS` だけでは既存表に列が増えず、status index の作成時に
/// 起動できなくなる。既存行はすべて適用済み mutation なので `applied` として移行する。
fn upgrade_local_mutations(tx: &rusqlite::Transaction<'_>) -> Result<()> {
    let table_exists: bool = tx.query_row(
        "SELECT EXISTS(
             SELECT 1 FROM sqlite_master
              WHERE type = 'table' AND name = 'local_mutations'
         )",
        [],
        |row| row.get(0),
    )?;
    if !table_exists {
        return Ok(());
    }

    let existing_columns = {
        let mut statement = tx.prepare("PRAGMA table_info(local_mutations)")?;
        statement
            .query_map([], |row| row.get::<_, String>(1))?
            .collect::<rusqlite::Result<Vec<_>>>()?
    };
    const ADDED_COLUMNS: [(&str, &str); 2] = [
        ("actor", "TEXT NOT NULL DEFAULT 'local'"),
        (
            "status",
            "TEXT NOT NULL DEFAULT 'applied' CHECK (status IN ('applied', 'conflict'))",
        ),
    ];
    for (name, definition) in ADDED_COLUMNS {
        if !existing_columns
            .iter()
            .any(|column| column.eq_ignore_ascii_case(name))
        {
            tx.execute_batch(&format!(
                "ALTER TABLE local_mutations ADD COLUMN {name} {definition};"
            ))?;
        }
    }
    Ok(())
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
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                document.id,
                document.workspace_id,
                document.title,
                document.body_text,
                document.blob_uri,
                document.document_type,
                document.created_at,
                document.updated_at,
            ],
        )?;
        Ok(())
    }

    pub fn update_document(tx: &rusqlite::Transaction<'_>, document: &DocumentAsset) -> Result<()> {
        let changed = tx.execute(
            "UPDATE documents SET title=?2, body_text=?3, updated_at=?4
             WHERE id=?1 AND workspace_id=?5 AND document_type=?6",
            params![
                document.id,
                document.title,
                document.body_text,
                document.updated_at,
                document.workspace_id,
                DOCUMENT_TYPE_MEMO
            ],
        )?;
        anyhow::ensure!(changed == 1, "追記先のメモが見つかりません");
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

    fn update_document(&mut self, document: &DocumentAsset) -> Result<()> {
        ledger_sql::update_document(self.tx, document)
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

    fn clear_document_metas(&mut self, document_id: &str) -> Result<()> {
        self.tx.execute(
            "DELETE FROM document_meta WHERE document_id=?1",
            params![document_id],
        )?;
        self.tx.execute(
            "DELETE FROM tag_assignments WHERE document_id=?1",
            params![document_id],
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
}

impl MutationStore for Database {
    fn apply_mutation(
        &self,
        request: &MutationRequest,
        recorded_at: &str,
    ) -> Result<MutationResult> {
        let entity_kind = request.operation.entity_kind();
        let entity_id = request
            .operation
            .entity_id(&request.workspace_id)
            .context("mutation の entity ID が確定していません")?;
        let operation_kind = request.operation.operation_kind();
        let payload_json = serde_json::to_string(&request.operation)?;

        let mut conn = self.conn.borrow_mut();
        // FullOS は mutation ごとに agentos sidecar を起動する。deferred transaction の
        // read -> write upgrade が競合すると busy_timeout を待たず SQLITE_BUSY になり得るため、
        // 最初に書き込み予約を取り、別 writer とは busy_timeout の範囲で直列化する。
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;

        let prior = tx
            .query_row(
                "SELECT workspace_id, entity_kind, entity_id, operation_kind, status, payload_json,
                        base_revision, resulting_revision, created_at
                   FROM local_mutations WHERE operation_id = ?1",
                params![request.operation_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, String>(4)?,
                        row.get::<_, String>(5)?,
                        row.get::<_, Option<i64>>(6)?,
                        row.get::<_, i64>(7)?,
                        row.get::<_, String>(8)?,
                    ))
                },
            )
            .optional()?;
        if let Some((workspace, kind, id, operation, status, payload, base, revision, created_at)) =
            prior
        {
            ensure!(
                workspace == request.workspace_id
                    && kind == entity_kind
                    && id == entity_id
                    && operation == operation_kind
                    && payload == payload_json
                    && base == request.base_revision,
                "operationId `{}` は別の mutation ですでに使われています",
                request.operation_id
            );
            return Ok(MutationResult {
                operation_id: request.operation_id.clone(),
                status: match status.as_str() {
                    "applied" => MutationStatus::Duplicate,
                    "conflict" => MutationStatus::Conflict,
                    other => anyhow::bail!("未知の mutation status です: {other}"),
                },
                entity_kind: kind,
                entity_id: id,
                revision,
                recorded_at: created_at,
            });
        }

        let current_revision = tx
            .query_row(
                "SELECT revision FROM entity_revisions
                  WHERE workspace_id = ?1 AND entity_kind = ?2 AND entity_id = ?3",
                params![request.workspace_id, entity_kind, entity_id],
                |row| row.get::<_, i64>(0),
            )
            .optional()?
            .unwrap_or(0);

        if request
            .base_revision
            .is_some_and(|base| base != current_revision)
        {
            tx.execute(
                "INSERT INTO local_mutations
                     (operation_id, workspace_id, entity_kind, entity_id, actor, operation_kind,
                      status, payload_json, base_revision, resulting_revision, created_at)
                 VALUES (?1, ?2, ?3, ?4, 'local', ?5, 'conflict', ?6, ?7, ?8, ?9)",
                params![
                    request.operation_id,
                    request.workspace_id,
                    entity_kind,
                    entity_id,
                    operation_kind,
                    payload_json,
                    request.base_revision,
                    current_revision,
                    recorded_at,
                ],
            )?;
            tx.commit()?;
            return Ok(MutationResult {
                operation_id: request.operation_id.clone(),
                status: MutationStatus::Conflict,
                entity_kind: entity_kind.into(),
                entity_id: entity_id.into(),
                revision: current_revision,
                recorded_at: recorded_at.into(),
            });
        }

        apply_operation(&tx, &request.workspace_id, &request.operation, recorded_at)?;

        let revision = current_revision + 1;
        tx.execute(
            "INSERT INTO entity_revisions
                 (workspace_id, entity_kind, entity_id, revision, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(workspace_id, entity_kind, entity_id)
             DO UPDATE SET revision = excluded.revision, updated_at = excluded.updated_at",
            params![
                request.workspace_id,
                entity_kind,
                entity_id,
                revision,
                recorded_at
            ],
        )?;
        tx.execute(
            "INSERT INTO local_mutations
                 (operation_id, workspace_id, entity_kind, entity_id, actor, operation_kind,
                  status, payload_json, base_revision, resulting_revision, created_at)
             VALUES (?1, ?2, ?3, ?4, 'local', ?5, 'applied', ?6, ?7, ?8, ?9)",
            params![
                request.operation_id,
                request.workspace_id,
                entity_kind,
                entity_id,
                operation_kind,
                payload_json,
                request.base_revision,
                revision,
                recorded_at,
            ],
        )?;
        tx.commit()?;

        Ok(MutationResult {
            operation_id: request.operation_id.clone(),
            status: MutationStatus::Applied,
            entity_kind: entity_kind.into(),
            entity_id: entity_id.into(),
            revision,
            recorded_at: recorded_at.into(),
        })
    }
}

fn apply_operation(
    tx: &rusqlite::Transaction<'_>,
    workspace_id: &str,
    operation: &MutationOperation,
    now: &str,
) -> Result<()> {
    match operation {
        MutationOperation::MemoStatePatch { memo_id, patch } => {
            ensure_document_exists(tx, workspace_id, memo_id)?;
            if let Some(done) = patch.done {
                tx.execute(
                    "INSERT INTO document_states
                         (document_id, workspace_id, done, done_at, updated_at)
                     VALUES (?1, ?2, ?3, ?4, ?5)
                     ON CONFLICT(document_id) DO UPDATE
                       SET done = excluded.done,
                           done_at = excluded.done_at,
                           updated_at = excluded.updated_at",
                    params![memo_id, workspace_id, done, done.then_some(now), now],
                )?;
            }
            if let Some(archived) = patch.archived {
                tx.execute(
                    "INSERT INTO document_states
                         (document_id, workspace_id, archived_at, updated_at)
                     VALUES (?1, ?2, ?3, ?4)
                     ON CONFLICT(document_id) DO UPDATE
                       SET archived_at = excluded.archived_at,
                           updated_at = excluded.updated_at",
                    params![memo_id, workspace_id, archived.then_some(now), now],
                )?;
            }
            if let Some(trashed) = patch.trashed {
                tx.execute(
                    "INSERT INTO document_states
                         (document_id, workspace_id, deleted_at, updated_at)
                     VALUES (?1, ?2, ?3, ?4)
                     ON CONFLICT(document_id) DO UPDATE
                       SET deleted_at = excluded.deleted_at,
                           updated_at = excluded.updated_at",
                    params![memo_id, workspace_id, trashed.then_some(now), now],
                )?;
            }
        }
        MutationOperation::ArchiveCompletedTasks { labels } => {
            let placeholders = std::iter::repeat_n("?", labels.len())
                .collect::<Vec<_>>()
                .join(", ");
            let sql = format!(
                "UPDATE document_states
                    SET archived_at = ?1, updated_at = ?1
                  WHERE workspace_id = ?2
                    AND done = 1
                    AND archived_at IS NULL
                    AND deleted_at IS NULL
                    AND document_id IN (
                      SELECT meta.document_id
                        FROM document_meta AS meta
                        JOIN documents AS document ON document.id = meta.document_id
                       WHERE document.workspace_id = ?2
                         AND document.document_type = ?3
                         AND lower(meta.label) IN ({placeholders})
                    )"
            );
            let mut values = vec![
                rusqlite::types::Value::Text(now.into()),
                rusqlite::types::Value::Text(workspace_id.into()),
                rusqlite::types::Value::Text(DOCUMENT_TYPE_MEMO.into()),
            ];
            values.extend(
                labels
                    .iter()
                    .map(|label| rusqlite::types::Value::Text(label.to_lowercase())),
            );
            tx.execute(&sql, params_from_iter(values))?;
        }
        MutationOperation::TagPatch { tag_id, patch } => {
            let kind = require_tag_kind(tx, workspace_id, tag_id)?;
            if let Some(display_name) = &patch.display_name {
                ensure!(
                    kind == "user",
                    "組み込みタグの displayName は変更できません"
                );
                tx.execute(
                    "UPDATE tag_definitions SET display_name = ?2 WHERE id = ?1",
                    params![tag_id, display_name],
                )?;
            }
            match &patch.shorthand {
                NullablePatch::Unchanged => {}
                NullablePatch::Clear => {
                    tx.execute(
                        "UPDATE tag_definitions SET shorthand = NULL WHERE id = ?1",
                        params![tag_id],
                    )?;
                }
                NullablePatch::Set(value) => {
                    tx.execute(
                        "UPDATE tag_definitions SET shorthand = ?2 WHERE id = ?1",
                        params![tag_id, value],
                    )?;
                }
            }
            if let Some(enabled) = patch.enabled {
                tx.execute(
                    "UPDATE tag_definitions SET enabled = ?2 WHERE id = ?1",
                    params![tag_id, enabled],
                )?;
            }
            match &patch.view {
                NullablePatch::Unchanged => {}
                NullablePatch::Clear => {
                    tx.execute(
                        "DELETE FROM view_bindings WHERE tag_id = ?1",
                        params![tag_id],
                    )?;
                }
                NullablePatch::Set(view) => {
                    tx.execute(
                        "INSERT INTO view_bindings(tag_id, view_id, updated_at)
                         VALUES (?1, ?2, ?3)
                         ON CONFLICT(tag_id) DO UPDATE
                           SET view_id = excluded.view_id, updated_at = excluded.updated_at",
                        params![tag_id, view, now],
                    )?;
                }
            }
            match &patch.recipe {
                NullablePatch::Unchanged => {}
                NullablePatch::Clear => {
                    tx.execute(
                        "DELETE FROM automation_bindings WHERE tag_id = ?1",
                        params![tag_id],
                    )?;
                }
                NullablePatch::Set(recipe) => {
                    let ownership = if recipe.managed {
                        "managed"
                    } else {
                        "external"
                    };
                    tx.execute(
                        "INSERT INTO automation_bindings
                             (tag_id, recipe_name, ownership, enabled, updated_at)
                         VALUES (?1, ?2, ?3, 1, ?4)
                         ON CONFLICT(tag_id) DO UPDATE
                           SET recipe_name = excluded.recipe_name,
                               ownership = excluded.ownership,
                               enabled = excluded.enabled,
                               updated_at = excluded.updated_at",
                        params![tag_id, recipe.name, ownership, now],
                    )?;
                }
            }
            tx.execute(
                "UPDATE tag_definitions SET updated_at = ?2 WHERE id = ?1",
                params![tag_id, now],
            )?;
        }
        MutationOperation::TagDelete { tag_id } => {
            require_tag_kind(tx, workspace_id, tag_id)?;
            let changed = tx.execute(
                "UPDATE tag_definitions
                    SET deleted_at = ?2, enabled = 0, updated_at = ?2
                  WHERE id = ?1 AND kind = 'user'",
                params![tag_id, now],
            )?;
            ensure!(changed == 1, "組み込みタグは削除できません");
        }
        MutationOperation::AutomationRuleCreate { rule_id, input } => {
            let rule_id = rule_id
                .as_deref()
                .context("automation rule ID が確定していません")?;
            tx.execute(
                "INSERT INTO automation_rules
                     (id, workspace_id, name, description, prompt, backend_kind, backend_config,
                      trigger_kind, trigger_config, enabled, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?11)",
                params![
                    rule_id,
                    workspace_id,
                    input.name,
                    input.description,
                    input.prompt,
                    input.backend.as_str(),
                    serde_json::to_string(&input.backend_config)?,
                    input.trigger_kind.as_str(),
                    serde_json::to_string(&input.trigger)?,
                    input.enabled,
                    now,
                ],
            )?;
        }
        MutationOperation::AutomationRulePatch { rule_id, patch } => {
            require_automation_rule(tx, workspace_id, rule_id)?;
            if let Some(name) = &patch.name {
                tx.execute(
                    "UPDATE automation_rules SET name = ?2 WHERE id = ?1",
                    params![rule_id, name],
                )?;
            }
            match &patch.description {
                NullablePatch::Unchanged => {}
                NullablePatch::Clear => {
                    tx.execute(
                        "UPDATE automation_rules SET description = NULL WHERE id = ?1",
                        params![rule_id],
                    )?;
                }
                NullablePatch::Set(value) => {
                    tx.execute(
                        "UPDATE automation_rules SET description = ?2 WHERE id = ?1",
                        params![rule_id, value],
                    )?;
                }
            }
            if let Some(prompt) = &patch.prompt {
                tx.execute(
                    "UPDATE automation_rules SET prompt = ?2 WHERE id = ?1",
                    params![rule_id, prompt],
                )?;
            }
            if let Some(backend) = patch.backend {
                tx.execute(
                    "UPDATE automation_rules SET backend_kind = ?2 WHERE id = ?1",
                    params![rule_id, backend.as_str()],
                )?;
            }
            if let Some(config) = &patch.backend_config {
                tx.execute(
                    "UPDATE automation_rules SET backend_config = ?2 WHERE id = ?1",
                    params![rule_id, serde_json::to_string(config)?],
                )?;
            }
            if let Some(kind) = patch.trigger_kind {
                tx.execute(
                    "UPDATE automation_rules SET trigger_kind = ?2 WHERE id = ?1",
                    params![rule_id, kind.as_str()],
                )?;
            }
            if let Some(trigger) = &patch.trigger {
                tx.execute(
                    "UPDATE automation_rules SET trigger_config = ?2 WHERE id = ?1",
                    params![rule_id, serde_json::to_string(trigger)?],
                )?;
            }
            if let Some(enabled) = patch.enabled {
                tx.execute(
                    "UPDATE automation_rules SET enabled = ?2 WHERE id = ?1",
                    params![rule_id, enabled],
                )?;
            }
            tx.execute(
                "UPDATE automation_rules SET updated_at = ?2 WHERE id = ?1",
                params![rule_id, now],
            )?;
            ensure_valid_automation_rule(tx, rule_id)?;
        }
        MutationOperation::AutomationRuleDelete { rule_id } => {
            require_automation_rule(tx, workspace_id, rule_id)?;
            tx.execute(
                "DELETE FROM automation_rules WHERE id = ?1 AND workspace_id = ?2",
                params![rule_id, workspace_id],
            )?;
        }
        MutationOperation::SettingSet { key, value } => {
            tx.execute(
                "INSERT INTO settings (workspace_id, key, value, updated_at)
                 VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(workspace_id, key)
                 DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
                params![workspace_id, key, value, now],
            )?;
        }
    }
    Ok(())
}

fn ensure_document_exists(
    tx: &rusqlite::Transaction<'_>,
    workspace_id: &str,
    document_id: &str,
) -> Result<()> {
    let exists: bool = tx.query_row(
        "SELECT EXISTS(
             SELECT 1 FROM documents
              WHERE id = ?1 AND workspace_id = ?2 AND document_type = ?3
         )",
        params![document_id, workspace_id, DOCUMENT_TYPE_MEMO],
        |row| row.get(0),
    )?;
    ensure!(exists, "メモが見つかりません: {document_id}");
    Ok(())
}

fn require_tag_kind(
    tx: &rusqlite::Transaction<'_>,
    workspace_id: &str,
    tag_id: &str,
) -> Result<String> {
    tx.query_row(
        "SELECT kind FROM tag_definitions
          WHERE id = ?1 AND (workspace_id = ?2 OR workspace_id = 'local')
            AND deleted_at IS NULL",
        params![tag_id, workspace_id],
        |row| row.get(0),
    )
    .optional()?
    .with_context(|| format!("タグが見つかりません: {tag_id}"))
}

fn require_automation_rule(
    tx: &rusqlite::Transaction<'_>,
    workspace_id: &str,
    rule_id: &str,
) -> Result<()> {
    let exists: bool = tx.query_row(
        "SELECT EXISTS(
             SELECT 1 FROM automation_rules WHERE id = ?1 AND workspace_id = ?2
         )",
        params![rule_id, workspace_id],
        |row| row.get(0),
    )?;
    ensure!(exists, "自動化ルールが見つかりません: {rule_id}");
    Ok(())
}

fn ensure_valid_automation_rule(tx: &rusqlite::Transaction<'_>, rule_id: &str) -> Result<()> {
    let (trigger_kind, trigger_json): (String, String) = tx.query_row(
        "SELECT trigger_kind, trigger_config FROM automation_rules WHERE id = ?1",
        params![rule_id],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    if trigger_kind == "schedule" {
        let trigger: Trigger =
            serde_json::from_str(&trigger_json).context("trigger_config を解釈できません")?;
        ensure!(
            trigger
                .cron
                .as_deref()
                .is_some_and(|cron| !cron.trim().is_empty()),
            "schedule には trigger.cron が必要です"
        );
    }
    Ok(())
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

    fn update_document(&mut self, document: &DocumentAsset) -> Result<()> {
        ledger_sql::update_document(self.tx, document)
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

    fn recent(&self, workspace_id: &str, limit: usize) -> Result<Vec<MemoSnapshot>> {
        let conn = self.conn.borrow();
        let mut statement = conn.prepare(
            "SELECT id, title, body_text, created_at FROM documents d
             WHERE workspace_id=?1 AND document_type=?2
               AND NOT EXISTS (SELECT 1 FROM document_states s
                 WHERE s.document_id=d.id AND s.deleted_at IS NOT NULL)
             ORDER BY updated_at DESC LIMIT ?3",
        )?;
        let rows = statement
            .query_map(
                params![workspace_id, DOCUMENT_TYPE_MEMO, limit as i64],
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
        rows.into_iter()
            .map(|(id, title, body, created_at)| {
                Ok(MemoSnapshot {
                    metas: metas_of(&conn, &id)?,
                    id,
                    title,
                    body_text: body.unwrap_or_default(),
                    created_at,
                })
            })
            .collect()
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

    const LEGACY_AUTOMATION_RUNS: &str = "CREATE TABLE automation_runs (
             id TEXT PRIMARY KEY,
             workspace_id TEXT NOT NULL,
             rule_id TEXT NOT NULL,
             source_document_id TEXT NOT NULL,
             result_document_id TEXT,
             status TEXT NOT NULL,
             backend_kind TEXT NOT NULL,
             error TEXT,
             started_at TEXT NOT NULL,
             finished_at TEXT
         )";

    const LEGACY_LOCAL_MUTATIONS: &str = "CREATE TABLE local_mutations (
             operation_id TEXT PRIMARY KEY,
             workspace_id TEXT NOT NULL,
             entity_kind TEXT NOT NULL,
             entity_id TEXT NOT NULL,
             operation_kind TEXT NOT NULL,
             payload_json TEXT NOT NULL,
             base_revision INTEGER,
             resulting_revision INTEGER NOT NULL,
             created_at TEXT NOT NULL
         );
         INSERT INTO local_mutations
             (operation_id, workspace_id, entity_kind, entity_id, operation_kind,
              payload_json, base_revision, resulting_revision, created_at)
         VALUES
             ('op-1', 'local', 'setting', 'key', 'setting_set', '{}', NULL, 1, 'old')";

    fn automation_run_columns(conn: &Connection) -> Vec<String> {
        let mut statement = conn.prepare("PRAGMA table_info(automation_runs)").unwrap();
        statement
            .query_map([], |row| row.get::<_, String>(1))
            .unwrap()
            .collect::<rusqlite::Result<Vec<_>>>()
            .unwrap()
    }

    #[test]
    fn upgrades_legacy_automation_runs_before_applying_schema() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(LEGACY_AUTOMATION_RUNS).unwrap();
        conn.execute(
            "INSERT INTO automation_runs
             (id, workspace_id, rule_id, source_document_id, status, backend_kind, started_at)
             VALUES ('run-1', 'ws', 'rule-1', 'doc-1', 'succeeded', 'api_key', 'now')",
            [],
        )
        .unwrap();

        let db = Database::from_connection(conn).unwrap();
        {
            let conn = db.conn.borrow();
            let columns = automation_run_columns(&conn);
            for name in [
                "tag_id",
                "recipe_name",
                "recipe_ownership",
                "processing_fingerprint",
                "input_fingerprint",
                "execution_key",
                "output_fingerprint",
                "forced",
            ] {
                assert!(
                    columns.iter().any(|column| column == name),
                    "missing {name}"
                );
            }
            let row: (String, Option<String>, i64) = conn
                .query_row(
                    "SELECT status, execution_key, forced FROM automation_runs WHERE id = 'run-1'",
                    [],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )
                .unwrap();
            assert_eq!(row, ("succeeded".into(), None, 0));
        }

        // Reopening an already upgraded database must be a no-op.
        Database::from_connection(db.conn.into_inner()).unwrap();
    }

    #[test]
    fn upgrades_legacy_local_mutations_before_creating_status_index() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(LEGACY_LOCAL_MUTATIONS).unwrap();

        let db = Database::from_connection(conn).unwrap();
        {
            let conn = db.conn.borrow();
            let receipt: (String, String) = conn
                .query_row(
                    "SELECT actor, status FROM local_mutations WHERE operation_id = 'op-1'",
                    [],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .unwrap();
            assert_eq!(receipt, ("local".into(), "applied".into()));
            let index_exists: bool = conn
                .query_row(
                    "SELECT EXISTS(
                         SELECT 1 FROM sqlite_master
                          WHERE type = 'index' AND name = 'idx_local_mutations_outbox'
                     )",
                    [],
                    |row| row.get(0),
                )
                .unwrap();
            assert!(index_exists);
        }

        // Receipt columns already exist, so reopening must not try to add them again.
        Database::from_connection(db.conn.into_inner()).unwrap();
    }

    #[test]
    fn rolls_back_legacy_upgrade_when_schema_application_fails() {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(LEGACY_AUTOMATION_RUNS).unwrap();

        let result = apply_schema(
            &mut conn,
            "CREATE TABLE should_be_rolled_back (id TEXT); THIS IS NOT SQL;",
        );

        assert!(result.is_err());
        assert_eq!(automation_run_columns(&conn).len(), 10);
        let marker_exists: bool = conn
            .query_row(
                "SELECT EXISTS(
                     SELECT 1 FROM sqlite_master
                     WHERE type = 'table' AND name = 'should_be_rolled_back'
                 )",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(!marker_exists);
    }

    #[test]
    fn opens_a_partially_upgraded_automation_runs_table() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(LEGACY_AUTOMATION_RUNS).unwrap();
        conn.execute_batch(
            "ALTER TABLE automation_runs ADD COLUMN tag_id TEXT;
             ALTER TABLE automation_runs ADD COLUMN forced INTEGER NOT NULL DEFAULT 0;",
        )
        .unwrap();

        let db = Database::from_connection(conn).unwrap();
        let conn = db.conn.borrow();
        let columns = automation_run_columns(&conn);
        assert_eq!(columns.len(), 18);
        assert!(
            conn.query_row(
                "SELECT 1 FROM sqlite_master
                 WHERE type = 'index' AND name = 'idx_automation_runs_execution'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .is_ok()
        );
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

        let records = LineageQuery::list(&db, "ws").unwrap();
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
        assert!(LineageQuery::list(&db, "ws").unwrap().is_empty());
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
        use crate::domain::mutation::{
            MutationOperation, MutationRequest, NullablePatch, TagPatch,
        };

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

        db.apply_mutation(
            &MutationRequest {
                operation_id: "op-rename".into(),
                workspace_id: "ws".into(),
                base_revision: Some(0),
                operation: MutationOperation::TagPatch {
                    tag_id: "tag-1".into(),
                    patch: TagPatch {
                        display_name: Some("new".into()),
                        shorthand: NullablePatch::Set("n".into()),
                        ..Default::default()
                    },
                },
            },
            "later",
        )
        .unwrap();
        let renamed = TagRepository::get(&db, "tag-1").unwrap().unwrap();
        assert_eq!(renamed.id, "tag-1");
        assert_eq!(renamed.display_name, "new");

        db.apply_mutation(
            &MutationRequest {
                operation_id: "op-delete".into(),
                workspace_id: "ws".into(),
                base_revision: Some(1),
                operation: MutationOperation::TagDelete {
                    tag_id: "tag-1".into(),
                },
            },
            "deleted",
        )
        .unwrap();
        let visible = TagRepository::list(&db, "ws", false).unwrap();
        assert!(visible.iter().all(|tag| tag.id != "tag-1"));
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

    #[test]
    fn memo_state_delta_is_idempotent_and_preserves_untouched_fields() {
        use crate::domain::mutation::{MemoStatePatch, MutationOperation, MutationRequest};

        let db = Database::open_in_memory().unwrap();
        db.conn
            .borrow()
            .execute(
                "INSERT INTO documents
                     (id, workspace_id, title, body_text, blob_uri, document_type, created_at, updated_at)
                 VALUES ('memo-1', 'ws', 'memo', 'body', NULL, 'memo', 'old', 'old')",
                [],
            )
            .unwrap();
        db.conn
            .borrow()
            .execute(
                "INSERT INTO document_states
                     (document_id, workspace_id, done, done_at, archived_at, deleted_at, updated_at)
                 VALUES ('memo-1', 'ws', 0, NULL, 'archived-before', NULL, 'old')",
                [],
            )
            .unwrap();

        let request = MutationRequest {
            operation_id: "op-memo".into(),
            workspace_id: "ws".into(),
            base_revision: Some(0),
            operation: MutationOperation::MemoStatePatch {
                memo_id: "memo-1".into(),
                patch: MemoStatePatch {
                    done: Some(true),
                    ..Default::default()
                },
            },
        };
        let applied = db.apply_mutation(&request, "now").unwrap();
        assert_eq!(applied.status, MutationStatus::Applied);
        assert_eq!(applied.revision, 1);

        let state: (i64, Option<String>, Option<String>) = db
            .conn
            .borrow()
            .query_row(
                "SELECT done, done_at, archived_at FROM document_states WHERE document_id='memo-1'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(
            state,
            (1, Some("now".into()), Some("archived-before".into()))
        );

        let duplicate = db.apply_mutation(&request, "later").unwrap();
        assert_eq!(duplicate.status, MutationStatus::Duplicate);
        assert_eq!(duplicate.revision, 1);
        assert_eq!(duplicate.recorded_at, "now");

        let stale = MutationRequest {
            operation_id: "op-stale".into(),
            workspace_id: "ws".into(),
            base_revision: Some(0),
            operation: MutationOperation::MemoStatePatch {
                memo_id: "memo-1".into(),
                patch: MemoStatePatch {
                    archived: Some(false),
                    ..Default::default()
                },
            },
        };
        let conflict = db.apply_mutation(&stale, "later").unwrap();
        assert_eq!(conflict.status, MutationStatus::Conflict);
        assert_eq!(conflict.revision, 1);
        let repeated_conflict = db.apply_mutation(&stale, "much-later").unwrap();
        assert_eq!(repeated_conflict.status, MutationStatus::Conflict);
        assert_eq!(repeated_conflict.revision, 1);
        assert_eq!(repeated_conflict.recorded_at, "later");

        let reused_id = MutationRequest {
            operation_id: "op-stale".into(),
            workspace_id: "ws".into(),
            base_revision: Some(1),
            operation: MutationOperation::MemoStatePatch {
                memo_id: "memo-1".into(),
                patch: MemoStatePatch {
                    archived: Some(false),
                    ..Default::default()
                },
            },
        };
        assert!(
            db.apply_mutation(&reused_id, "much-later")
                .unwrap_err()
                .to_string()
                .contains("別の mutation")
        );
        let archived: Option<String> = db
            .conn
            .borrow()
            .query_row(
                "SELECT archived_at FROM document_states WHERE document_id='memo-1'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(archived.as_deref(), Some("archived-before"));
    }

    #[test]
    fn memo_state_mutations_never_apply_to_non_memo_documents() {
        use crate::domain::mutation::{MemoStatePatch, MutationOperation, MutationRequest};

        let db = Database::open_in_memory().unwrap();
        db.conn
            .borrow()
            .execute(
                "INSERT INTO documents
                     (id, workspace_id, title, body_text, blob_uri, document_type, created_at, updated_at)
                 VALUES ('result-1', 'ws', 'result', 'body', NULL, 'automation_result', 'old', 'old')",
                [],
            )
            .unwrap();

        let request = MutationRequest {
            operation_id: "op-result".into(),
            workspace_id: "ws".into(),
            base_revision: Some(0),
            operation: MutationOperation::MemoStatePatch {
                memo_id: "result-1".into(),
                patch: MemoStatePatch {
                    done: Some(true),
                    ..Default::default()
                },
            },
        };

        let error = db.apply_mutation(&request, "now").unwrap_err();
        assert!(error.to_string().contains("メモが見つかりません"));
        let states: i64 = db
            .conn
            .borrow()
            .query_row("SELECT count(*) FROM document_states", [], |row| row.get(0))
            .unwrap();
        let mutations: i64 = db
            .conn
            .borrow()
            .query_row("SELECT count(*) FROM local_mutations", [], |row| row.get(0))
            .unwrap();
        assert_eq!((states, mutations), (0, 0));
    }

    #[test]
    fn archive_completed_tasks_only_archives_memos() {
        use crate::domain::mutation::{MutationOperation, MutationRequest};

        let db = Database::open_in_memory().unwrap();
        db.conn
            .borrow()
            .execute_batch(
                "INSERT INTO documents
                     (id, workspace_id, title, body_text, blob_uri, document_type, created_at, updated_at)
                 VALUES
                     ('memo-1', 'ws', 'memo', 'body', NULL, 'memo', 'old', 'old'),
                     ('result-1', 'ws', 'result', 'body', NULL, 'automation_result', 'old', 'old');
                 INSERT INTO document_states(document_id, workspace_id, done, updated_at)
                 VALUES ('memo-1', 'ws', 1, 'old'), ('result-1', 'ws', 1, 'old');
                 INSERT INTO document_meta(id, document_id, label, value, source, created_at)
                 VALUES
                     ('meta-1', 'memo-1', 'task', NULL, 'user', 'old'),
                     ('meta-2', 'result-1', 'task', NULL, 'user', 'old');",
            )
            .unwrap();

        let request = MutationRequest {
            operation_id: "op-archive".into(),
            workspace_id: "ws".into(),
            base_revision: Some(0),
            operation: MutationOperation::ArchiveCompletedTasks {
                labels: vec!["task".into()],
            },
        };
        db.apply_mutation(&request, "now").unwrap();

        let states: (Option<String>, Option<String>) = db
            .conn
            .borrow()
            .query_row(
                "SELECT
                     max(CASE WHEN document_id = 'memo-1' THEN archived_at END),
                     max(CASE WHEN document_id = 'result-1' THEN archived_at END)
                   FROM document_states",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(states, (Some("now".into()), None));
    }

    #[test]
    fn tag_delta_updates_only_supplied_fields_and_bindings_atomically() {
        use crate::domain::mutation::{
            MutationOperation, MutationRequest, NullablePatch, TagPatch, TagRecipe,
        };

        let db = Database::open_in_memory().unwrap();
        db.conn
            .borrow()
            .execute(
                "INSERT INTO tag_definitions
                 VALUES('tag-1','ws','user','old','o',1,0,1,NULL,NULL,'old','old')",
                [],
            )
            .unwrap();
        db.conn
            .borrow()
            .execute(
                "INSERT INTO view_bindings VALUES('tag-1','existing-view','old')",
                [],
            )
            .unwrap();

        let first = MutationRequest {
            operation_id: "op-tag-1".into(),
            workspace_id: "ws".into(),
            base_revision: Some(0),
            operation: MutationOperation::TagPatch {
                tag_id: "tag-1".into(),
                patch: TagPatch {
                    enabled: Some(false),
                    shorthand: NullablePatch::Clear,
                    ..Default::default()
                },
            },
        };
        assert_eq!(db.apply_mutation(&first, "one").unwrap().revision, 1);
        let row: (String, Option<String>, i64, String) = db
            .conn
            .borrow()
            .query_row(
                "SELECT t.display_name, t.shorthand, t.enabled, v.view_id
                   FROM tag_definitions t JOIN view_bindings v ON v.tag_id=t.id
                  WHERE t.id='tag-1'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .unwrap();
        assert_eq!(row, ("old".into(), None, 0, "existing-view".into()));

        let second = MutationRequest {
            operation_id: "op-tag-2".into(),
            workspace_id: "ws".into(),
            base_revision: Some(1),
            operation: MutationOperation::TagPatch {
                tag_id: "tag-1".into(),
                patch: TagPatch {
                    view: NullablePatch::Clear,
                    recipe: NullablePatch::Set(TagRecipe {
                        name: "build".into(),
                        managed: true,
                    }),
                    ..Default::default()
                },
            },
        };
        assert_eq!(db.apply_mutation(&second, "two").unwrap().revision, 2);
        let view_count: i64 = db
            .conn
            .borrow()
            .query_row(
                "SELECT count(*) FROM view_bindings WHERE tag_id='tag-1'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let recipe: (String, String) = db
            .conn
            .borrow()
            .query_row(
                "SELECT recipe_name, ownership FROM automation_bindings WHERE tag_id='tag-1'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(view_count, 0);
        assert_eq!(recipe, ("build".into(), "managed".into()));
    }

    #[test]
    fn automation_rule_patch_does_not_replace_the_whole_rule() {
        use crate::domain::automation::{BackendConfig, BackendKind, Trigger, TriggerKind};
        use crate::domain::mutation::{
            AutomationRuleInput, AutomationRulePatch, MutationOperation, MutationRequest,
        };

        let db = Database::open_in_memory().unwrap();
        let create = MutationRequest {
            operation_id: "op-create".into(),
            workspace_id: "ws".into(),
            base_revision: Some(0),
            operation: MutationOperation::AutomationRuleCreate {
                rule_id: Some("rule-1".into()),
                input: AutomationRuleInput {
                    name: "name".into(),
                    description: Some("description".into()),
                    prompt: "keep this prompt".into(),
                    backend: BackendKind::ApiKey,
                    backend_config: BackendConfig {
                        provider: "anthropic".into(),
                        ..Default::default()
                    },
                    trigger_kind: TriggerKind::Manual,
                    trigger: Trigger::default(),
                    enabled: true,
                },
            },
        };
        db.apply_mutation(&create, "created").unwrap();

        let patch = MutationRequest {
            operation_id: "op-patch".into(),
            workspace_id: "ws".into(),
            base_revision: Some(1),
            operation: MutationOperation::AutomationRulePatch {
                rule_id: "rule-1".into(),
                patch: AutomationRulePatch {
                    enabled: Some(false),
                    ..Default::default()
                },
            },
        };
        let result = db.apply_mutation(&patch, "patched").unwrap();
        assert_eq!(result.revision, 2);
        let row: (String, String, i64, String) = db
            .conn
            .borrow()
            .query_row(
                "SELECT name, prompt, enabled, updated_at FROM automation_rules WHERE id='rule-1'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .unwrap();
        assert_eq!(
            row,
            (
                "name".into(),
                "keep this prompt".into(),
                0,
                "patched".into()
            )
        );
    }
}
