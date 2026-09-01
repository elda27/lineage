//! Versioned migrations for the local SQLite structured-state store.
//!
//! The local database deliberately owns its migration chain independently from
//! Cloudflare D1. Released migration files are append-only and must never be
//! edited in place.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, bail, ensure};
use rusqlite::{Connection, TransactionBehavior, backup, params};
use rusqlite_migration::{M, Migrations};
use serde::Serialize;

const BASELINE_SCHEMA_VERSION: i64 = 1;
pub(crate) const LATEST_SCHEMA_VERSION: i64 = 1;

const INITIAL_SCHEMA: &str = include_str!("sqlite_migrations/0001_initial.sql");
const MIGRATIONS: Migrations<'static> = Migrations::from_slice(&[M::up(INITIAL_SCHEMA)]);

const AUTOMATION_RUNS_LEGACY_COLUMNS: [&str; 10] = [
    "id",
    "workspace_id",
    "rule_id",
    "source_document_id",
    "result_document_id",
    "status",
    "backend_kind",
    "error",
    "started_at",
    "finished_at",
];

const AUTOMATION_RUNS_ADDED_COLUMNS: [(&str, &str); 8] = [
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

const LOCAL_MUTATIONS_LEGACY_COLUMNS: [&str; 9] = [
    "operation_id",
    "workspace_id",
    "entity_kind",
    "entity_id",
    "operation_kind",
    "payload_json",
    "base_revision",
    "resulting_revision",
    "created_at",
];

const LOCAL_MUTATIONS_ADDED_COLUMNS: [(&str, &str); 2] = [
    (
        "actor",
        "ALTER TABLE local_mutations ADD COLUMN actor TEXT NOT NULL DEFAULT 'local'",
    ),
    (
        "status",
        "ALTER TABLE local_mutations ADD COLUMN status TEXT NOT NULL DEFAULT 'applied' CHECK (status IN ('applied', 'conflict'))",
    ),
];

/// Result of checking and, when necessary, migrating one local database.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MigrationReport {
    pub from_version: i64,
    pub to_version: i64,
    pub applied_migrations: i64,
    pub backup_path: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ColumnShape {
    declared_type: String,
    not_null: bool,
    default_value: Option<String>,
    primary_key_position: i64,
}

type TableShape = BTreeMap<String, ColumnShape>;
type SchemaShape = BTreeMap<String, TableShape>;

pub(crate) fn open_and_migrate(path: &Path) -> Result<(Connection, MigrationReport)> {
    let existed_before_open = path.exists();
    let mut conn =
        Connection::open(path).with_context(|| format!("DB を開けません: {}", path.display()))?;
    conn.busy_timeout(Duration::from_secs(3))?;
    let report = migrate_connection(&mut conn, Some((path, existed_before_open)))?;
    Ok((conn, report))
}

pub(crate) fn migrate_unbacked_connection(conn: &mut Connection) -> Result<MigrationReport> {
    conn.busy_timeout(Duration::from_secs(3))?;
    migrate_connection(conn, None)
}

fn migrate_connection(
    conn: &mut Connection,
    file: Option<(&Path, bool)>,
) -> Result<MigrationReport> {
    let from_version = user_version(conn)?;
    ensure!(
        from_version <= LATEST_SCHEMA_VERSION,
        "DB スキーマのバージョン {from_version} は、このアプリが対応する最新バージョン {LATEST_SCHEMA_VERSION} より新しいため開けません"
    );

    let actual_schema = read_schema_shape(conn)?;
    reject_unexpected_views_or_triggers(conn)?;
    let has_user_tables = !actual_schema.is_empty();
    if from_version == 0 && has_user_tables {
        validate_unversioned_schema(&actual_schema)?;
    }

    if from_version == LATEST_SCHEMA_VERSION {
        let expected_schema = expected_schema_shape()?;
        validate_versioned_schema(&actual_schema, &expected_schema)?;
        return Ok(MigrationReport {
            from_version,
            to_version: from_version,
            applied_migrations: 0,
            backup_path: None,
        });
    }

    let backup_path = match file {
        Some((path, true)) => Some(create_backup(
            conn,
            path,
            from_version,
            LATEST_SCHEMA_VERSION,
        )?),
        _ => None,
    };

    if from_version == 0 && has_user_tables {
        adopt_unversioned_schema(conn, INITIAL_SCHEMA)?;
    }

    MIGRATIONS
        .to_latest(conn)
        .context("ローカル SQLite migration の適用に失敗しました")?;

    let to_version = user_version(conn)?;
    ensure!(
        to_version == LATEST_SCHEMA_VERSION,
        "migration 後の DB バージョンが不正です: expected={LATEST_SCHEMA_VERSION}, actual={to_version}"
    );
    let migrated_schema = read_schema_shape(conn)?;
    let expected_schema = expected_schema_shape()?;
    validate_versioned_schema(&migrated_schema, &expected_schema)
        .context("migration 後のローカル schema 検証に失敗しました")?;

    Ok(MigrationReport {
        from_version,
        to_version,
        applied_migrations: to_version - from_version,
        backup_path,
    })
}

fn user_version(conn: &Connection) -> Result<i64> {
    conn.pragma_query_value(None, "user_version", |row| row.get(0))
        .context("PRAGMA user_version を読めません")
}

fn adopt_unversioned_schema(conn: &mut Connection, schema: &str) -> Result<()> {
    let tx = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .context("既存 DB の baseline transaction を開始できません")?;

    add_missing_columns(&tx, "automation_runs", &AUTOMATION_RUNS_ADDED_COLUMNS)?;
    add_missing_columns(&tx, "local_mutations", &LOCAL_MUTATIONS_ADDED_COLUMNS)?;
    tx.execute_batch(schema)
        .context("既存 DB へ v0.0.9 baseline schema を適用できません")?;
    tx.execute_batch(&format!("PRAGMA user_version = {BASELINE_SCHEMA_VERSION};"))
        .context("既存 DB の baseline version を記録できません")?;
    tx.commit()
        .context("既存 DB の baseline transaction を確定できません")?;
    Ok(())
}

fn add_missing_columns(
    tx: &rusqlite::Transaction<'_>,
    table: &str,
    columns: &[(&str, &str)],
) -> Result<()> {
    if !table_exists(tx, table)? {
        return Ok(());
    }

    let existing = table_columns(tx, table)?;
    for (name, statement) in columns {
        if !existing.contains_key(*name) {
            tx.execute_batch(statement)
                .with_context(|| format!("{table}.{name} を追加できません"))?;
        }
    }
    Ok(())
}

fn table_exists(conn: &Connection, table: &str) -> Result<bool> {
    conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE type = 'table' AND name = ?1)",
        params![table],
        |row| row.get(0),
    )
    .with_context(|| format!("テーブル {table} の存在を確認できません"))
}

fn validate_unversioned_schema(actual: &SchemaShape) -> Result<()> {
    let expected = expected_schema_shape()?;

    for (table, actual_columns) in actual {
        let Some(expected_columns) = expected.get(table) else {
            bail!("未対応のテーブル `{table}` を含む user_version=0 の DB は変更せず拒否します");
        };

        match table.as_str() {
            "automation_runs" => validate_legacy_compatible_table(
                table,
                actual_columns,
                expected_columns,
                &AUTOMATION_RUNS_LEGACY_COLUMNS,
            )?,
            "local_mutations" => validate_legacy_compatible_table(
                table,
                actual_columns,
                expected_columns,
                &LOCAL_MUTATIONS_LEGACY_COLUMNS,
            )?,
            _ => ensure!(
                actual_columns == expected_columns,
                "未対応の schema shape を持つテーブル `{table}` を含む user_version=0 の DB は変更せず拒否します"
            ),
        }
    }
    Ok(())
}

fn validate_legacy_compatible_table(
    table: &str,
    actual: &TableShape,
    expected: &TableShape,
    required_legacy_columns: &[&str],
) -> Result<()> {
    for required in required_legacy_columns {
        ensure!(
            actual.contains_key(*required),
            "テーブル `{table}` に必須の旧 schema 列 `{required}` がありません"
        );
    }
    for (name, shape) in actual {
        let Some(expected_shape) = expected.get(name) else {
            bail!("テーブル `{table}` に未対応の列 `{name}` があります");
        };
        ensure!(
            column_shape_matches(table, name, shape, expected_shape),
            "テーブル `{table}` の列 `{name}` が既知の schema shape と一致しません"
        );
    }
    Ok(())
}

fn validate_versioned_schema(actual: &SchemaShape, expected: &SchemaShape) -> Result<()> {
    ensure!(
        actual.len() == expected.len(),
        "DB は schema version {LATEST_SCHEMA_VERSION} を名乗っていますが、テーブル集合が既知の schema shape と一致しません"
    );

    for (table, expected_columns) in expected {
        let actual_columns = actual.get(table).with_context(|| {
            format!(
                "schema version {LATEST_SCHEMA_VERSION} の DB に必須テーブル `{table}` がありません"
            )
        })?;
        ensure!(
            actual_columns.len() == expected_columns.len(),
            "schema version {LATEST_SCHEMA_VERSION} のテーブル `{table}` の列集合が既知の schema shape と一致しません"
        );
        for (name, expected_shape) in expected_columns {
            let actual_shape = actual_columns.get(name).with_context(|| {
                format!("schema version {LATEST_SCHEMA_VERSION} のテーブル `{table}` に必須列 `{name}` がありません")
            })?;
            ensure!(
                column_shape_matches(table, name, actual_shape, expected_shape),
                "schema version {LATEST_SCHEMA_VERSION} のテーブル `{table}` の列 `{name}` が既知の schema shape と一致しません"
            );
        }
    }
    Ok(())
}

fn column_shape_matches(
    table: &str,
    column: &str,
    actual: &ColumnShape,
    expected: &ColumnShape,
) -> bool {
    if actual == expected {
        return true;
    }

    let same_except_default = actual.declared_type == expected.declared_type
        && actual.not_null == expected.not_null
        && actual.primary_key_position == expected.primary_key_position;
    if !same_except_default || expected.default_value.is_some() {
        return false;
    }

    // The transitional compatibility repair added defaults so existing rows
    // could be populated, while the then-current CREATE TABLE declaration
    // omitted them. Both are released, deterministic local schema variants.
    matches!(
        (table, column, actual.default_value.as_deref()),
        ("local_mutations", "actor", Some("'local'"))
            | ("local_mutations", "status", Some("'applied'"))
    )
}

fn expected_schema_shape() -> Result<SchemaShape> {
    let mut conn = Connection::open_in_memory()?;
    MIGRATIONS
        .to_latest(&mut conn)
        .context("expected local schema を構築できません")?;
    read_schema_shape(&conn)
}

fn read_schema_shape(conn: &Connection) -> Result<SchemaShape> {
    let mut statement = conn.prepare(
        "SELECT name FROM sqlite_schema
         WHERE type = 'table' AND name NOT LIKE 'sqlite_%'
         ORDER BY name",
    )?;
    let names = statement
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    names
        .into_iter()
        .map(|name| {
            let columns = table_columns(conn, &name)?;
            Ok((name, columns))
        })
        .collect()
}

fn table_columns(conn: &Connection, table: &str) -> Result<TableShape> {
    let mut statement = conn.prepare(
        "SELECT name, type, \"notnull\", dflt_value, pk
         FROM pragma_table_info(?1)
         ORDER BY cid",
    )?;
    let rows = statement.query_map(params![table], |row| {
        Ok((
            row.get::<_, String>(0)?,
            ColumnShape {
                declared_type: row.get::<_, String>(1)?.to_ascii_uppercase(),
                not_null: row.get::<_, i64>(2)? != 0,
                default_value: row.get(3)?,
                primary_key_position: row.get(4)?,
            },
        ))
    })?;
    Ok(rows.collect::<rusqlite::Result<BTreeMap<_, _>>>()?)
}

fn reject_unexpected_views_or_triggers(conn: &Connection) -> Result<()> {
    let mut statement = conn.prepare(
        "SELECT type, name FROM sqlite_schema
         WHERE type IN ('view', 'trigger') AND name NOT LIKE 'sqlite_%'
         ORDER BY type, name",
    )?;
    let objects = statement
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    ensure!(
        objects.is_empty(),
        "未対応の view/trigger を含む DB は変更せず拒否します: {objects:?}"
    );
    Ok(())
}

fn create_backup(
    source: &Connection,
    database_path: &Path,
    from_version: i64,
    to_version: i64,
) -> Result<PathBuf> {
    let backup_path = next_backup_path(database_path, from_version, to_version)?;
    let mut destination = Connection::open(&backup_path).with_context(|| {
        format!(
            "migration backup を作成できません: {}",
            backup_path.display()
        )
    })?;
    let backup = backup::Backup::new(source, &mut destination)
        .context("SQLite online backup を開始できません")?;
    backup
        .run_to_completion(64, Duration::from_millis(5), None)
        .context("SQLite online backup を完了できません")?;
    drop(backup);
    let integrity: String = destination
        .query_row("PRAGMA quick_check", [], |row| row.get(0))
        .context("migration backup の整合性を確認できません")?;
    ensure!(
        integrity == "ok",
        "migration backup の整合性検査に失敗しました: {integrity}"
    );
    Ok(backup_path)
}

fn next_backup_path(database_path: &Path, from_version: i64, to_version: i64) -> Result<PathBuf> {
    let file_name = database_path
        .file_name()
        .context("DB ファイル名を特定できません")?
        .to_string_lossy();
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();

    for attempt in 0..1000_u16 {
        let suffix = if attempt == 0 {
            String::new()
        } else {
            format!("-{attempt}")
        };
        let candidate = database_path.with_file_name(format!(
            "{file_name}.pre-migration-v{from_version}-to-v{to_version}-{timestamp}{suffix}.bak"
        ));
        if !candidate.exists() {
            return Ok(candidate);
        }
    }
    bail!("migration backup の一意なファイル名を確保できません")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;
    use std::fs;

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
         )";

    fn unique_path(label: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("lineage-{label}-{}-{nonce}.db", std::process::id()))
    }

    fn cleanup(path: &Path) {
        let _ = fs::remove_file(path);
        let _ = fs::remove_file(path.with_extension("db-wal"));
        let _ = fs::remove_file(path.with_extension("db-shm"));
        if let (Some(parent), Some(file_name)) = (path.parent(), path.file_name()) {
            let prefix = format!("{}.pre-migration-", file_name.to_string_lossy());
            if let Ok(entries) = fs::read_dir(parent) {
                for entry in entries.flatten() {
                    if entry.file_name().to_string_lossy().starts_with(&prefix) {
                        let _ = fs::remove_file(entry.path());
                    }
                }
            }
        }
    }

    #[test]
    fn migration_chain_is_valid() {
        MIGRATIONS.validate().unwrap();
    }

    #[test]
    fn empty_database_reaches_latest_version() {
        let mut conn = Connection::open_in_memory().unwrap();
        let report = migrate_unbacked_connection(&mut conn).unwrap();
        assert_eq!(report.from_version, 0);
        assert_eq!(report.to_version, LATEST_SCHEMA_VERSION);
        assert!(table_exists(&conn, "documents").unwrap());
        assert!(report.backup_path.is_none());
    }

    #[test]
    fn legacy_tables_are_adopted_without_losing_rows() {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(LEGACY_AUTOMATION_RUNS).unwrap();
        conn.execute_batch(LEGACY_LOCAL_MUTATIONS).unwrap();
        conn.execute(
            "INSERT INTO automation_runs
             (id, workspace_id, rule_id, source_document_id, status, backend_kind, started_at)
             VALUES ('run-1', 'ws', 'rule-1', 'doc-1', 'succeeded', 'api_key', 'now')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO local_mutations
             (operation_id, workspace_id, entity_kind, entity_id, operation_kind,
              payload_json, base_revision, resulting_revision, created_at)
             VALUES ('op-1', 'local', 'setting', 'key', 'setting_set', '{}', NULL, 1, 'old')",
            [],
        )
        .unwrap();

        migrate_unbacked_connection(&mut conn).unwrap();
        assert_eq!(table_columns(&conn, "automation_runs").unwrap().len(), 18);
        assert_eq!(table_columns(&conn, "local_mutations").unwrap().len(), 11);
        let run: (String, Option<String>, i64) = conn
            .query_row(
                "SELECT status, execution_key, forced FROM automation_runs WHERE id = 'run-1'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(run, ("succeeded".into(), None, 0));
        let mutation: (String, String) = conn
            .query_row(
                "SELECT actor, status FROM local_mutations WHERE operation_id = 'op-1'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(mutation, ("local".into(), "applied".into()));
    }

    #[test]
    fn partially_added_columns_are_completed() {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(LEGACY_AUTOMATION_RUNS).unwrap();
        conn.execute_batch(
            "ALTER TABLE automation_runs ADD COLUMN tag_id TEXT;
             ALTER TABLE automation_runs ADD COLUMN forced INTEGER NOT NULL DEFAULT 0;",
        )
        .unwrap();

        migrate_unbacked_connection(&mut conn).unwrap();
        assert_eq!(table_columns(&conn, "automation_runs").unwrap().len(), 18);
        assert!(
            conn.query_row(
                "SELECT 1 FROM sqlite_schema
                 WHERE type = 'index' AND name = 'idx_automation_runs_execution'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .is_ok()
        );
    }

    #[test]
    fn current_unversioned_database_is_baselined_without_duplicate_seed_rows() {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(INITIAL_SCHEMA).unwrap();
        assert_eq!(user_version(&conn).unwrap(), 0);
        let before: i64 = conn
            .query_row(
                "SELECT count(*) FROM tag_definitions WHERE id LIKE 'builtin:%'",
                [],
                |row| row.get(0),
            )
            .unwrap();

        migrate_unbacked_connection(&mut conn).unwrap();
        let after: i64 = conn
            .query_row(
                "SELECT count(*) FROM tag_definitions WHERE id LIKE 'builtin:%'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!((before, after), (2, 2));
    }

    #[test]
    fn database_repaired_by_the_transitional_receipt_upgrade_is_supported() {
        let mut conn = Connection::open_in_memory().unwrap();
        let repaired_schema = INITIAL_SCHEMA
            .replace(
                "actor TEXT NOT NULL,",
                "actor TEXT NOT NULL DEFAULT 'local',",
            )
            .replace(
                "status TEXT NOT NULL CHECK (status IN ('applied', 'conflict'))",
                "status TEXT NOT NULL DEFAULT 'applied' CHECK (status IN ('applied', 'conflict'))",
            );
        conn.execute_batch(&repaired_schema).unwrap();

        migrate_unbacked_connection(&mut conn).unwrap();
        let report = migrate_unbacked_connection(&mut conn).unwrap();
        assert_eq!(report.applied_migrations, 0);
        assert_eq!(user_version(&conn).unwrap(), LATEST_SCHEMA_VERSION);
    }

    #[test]
    fn reopening_current_database_is_a_no_op() {
        let mut conn = Connection::open_in_memory().unwrap();
        migrate_unbacked_connection(&mut conn).unwrap();
        let report = migrate_unbacked_connection(&mut conn).unwrap();
        assert_eq!(report.applied_migrations, 0);
        assert!(report.backup_path.is_none());
    }

    #[test]
    fn baseline_failure_rolls_back_columns_seed_data_and_version() {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(LEGACY_AUTOMATION_RUNS).unwrap();
        let result = adopt_unversioned_schema(
            &mut conn,
            "CREATE TABLE rolled_back(id TEXT); THIS IS NOT SQL;",
        );
        assert!(result.is_err());
        assert_eq!(table_columns(&conn, "automation_runs").unwrap().len(), 10);
        assert!(!table_exists(&conn, "rolled_back").unwrap());
        assert_eq!(user_version(&conn).unwrap(), 0);
    }

    #[test]
    fn unknown_schema_shape_is_rejected_without_mutation() {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE surprise(id TEXT); INSERT INTO surprise VALUES ('x');")
            .unwrap();
        let result = migrate_unbacked_connection(&mut conn);
        assert!(result.is_err());
        assert_eq!(user_version(&conn).unwrap(), 0);
        let value: String = conn
            .query_row("SELECT id FROM surprise", [], |row| row.get(0))
            .unwrap();
        assert_eq!(value, "x");
    }

    #[test]
    fn current_version_with_an_unknown_shape_is_rejected_without_mutation() {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE surprise(id TEXT);
             INSERT INTO surprise VALUES ('x');
             PRAGMA user_version = 1;",
        )
        .unwrap();

        let result = migrate_unbacked_connection(&mut conn);
        assert!(result.is_err());
        assert_eq!(user_version(&conn).unwrap(), 1);
        let value: String = conn
            .query_row("SELECT id FROM surprise", [], |row| row.get(0))
            .unwrap();
        assert_eq!(value, "x");
    }

    #[test]
    fn future_version_is_rejected_without_downgrade() {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA user_version = 99;").unwrap();
        let result = migrate_unbacked_connection(&mut conn);
        assert!(result.is_err());
        assert_eq!(user_version(&conn).unwrap(), 99);
    }

    #[test]
    fn existing_empty_file_is_backed_up_before_initial_migration() {
        let path = unique_path("empty-backup");
        cleanup(&path);
        Connection::open(&path).unwrap().close().unwrap();

        let (conn, report) = open_and_migrate(&path).unwrap();
        drop(conn);
        let backup_path = report.backup_path.expect("backup path");
        assert!(backup_path.is_file());
        let backup = Connection::open(&backup_path).unwrap();
        assert_eq!(user_version(&backup).unwrap(), 0);
        assert!(read_schema_shape(&backup).unwrap().is_empty());
        drop(backup);

        cleanup(&path);
        let _ = fs::remove_file(backup_path);
    }

    #[test]
    fn file_migration_creates_recoverable_online_backup() {
        let path = unique_path("backup");
        cleanup(&path);
        {
            let conn = Connection::open(&path).unwrap();
            conn.execute_batch(LEGACY_AUTOMATION_RUNS).unwrap();
            conn.execute(
                "INSERT INTO automation_runs
                 (id, workspace_id, rule_id, source_document_id, status, backend_kind, started_at)
                 VALUES ('run-1', 'ws', 'rule-1', 'doc-1', 'succeeded', 'api_key', 'now')",
                [],
            )
            .unwrap();
        }

        let (conn, report) = open_and_migrate(&path).unwrap();
        drop(conn);
        let backup_path = report.backup_path.expect("backup path");
        assert!(backup_path.is_file());
        let backup = Connection::open(&backup_path).unwrap();
        assert_eq!(user_version(&backup).unwrap(), 0);
        assert_eq!(table_columns(&backup, "automation_runs").unwrap().len(), 10);
        let id: String = backup
            .query_row("SELECT id FROM automation_runs", [], |row| row.get(0))
            .unwrap();
        assert_eq!(id, "run-1");
        drop(backup);

        let recovered_path = unique_path("recovered");
        fs::copy(&backup_path, &recovered_path).unwrap();
        let (recovered, _) = open_and_migrate(&recovered_path).unwrap();
        let recovered_id: String = recovered
            .query_row("SELECT id FROM automation_runs", [], |row| row.get(0))
            .unwrap();
        assert_eq!(recovered_id, "run-1");
        drop(recovered);

        cleanup(&path);
        cleanup(&recovered_path);
        let _ = fs::remove_file(backup_path);
    }

    #[test]
    fn current_rows_ids_counters_and_hash_chain_survive_baselining() {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(INITIAL_SCHEMA).unwrap();
        conn.execute(
            "INSERT INTO workspaces VALUES ('ws', 'workspace', NULL, 'now')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO documents VALUES ('doc-1', 'ws', 'title', 'body', NULL, 'memo', 'now', 'now')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO links VALUES
             ('link-1', 'ws', 1, 'minos', 'capture', 'document', 'doc-1',
              'derived_from', 'local', 'now', 'hash-1', '')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO meta_tags VALUES ('tag-1', 'ws', 'project', 'p', 7, 'now', 'now')",
            [],
        )
        .unwrap();

        migrate_unbacked_connection(&mut conn).unwrap();
        let document: String = conn
            .query_row("SELECT id FROM documents", [], |row| row.get(0))
            .unwrap();
        let link: (String, String, String) = conn
            .query_row("SELECT id, content_hash, prev_hash FROM links", [], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?))
            })
            .unwrap();
        let usage: i64 = conn
            .query_row(
                "SELECT usage_count FROM meta_tags WHERE id = 'tag-1'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(document, "doc-1");
        assert_eq!(link, ("link-1".into(), "hash-1".into(), "".into()));
        assert_eq!(usage, 7);
    }

    #[test]
    fn known_table_with_wrong_column_shape_is_rejected() {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE workspaces(id INTEGER PRIMARY KEY);")
            .unwrap();
        let result = migrate_unbacked_connection(&mut conn);
        assert!(result.is_err());
        assert_eq!(user_version(&conn).unwrap(), 0);
    }

    #[test]
    fn legacy_column_sets_are_complete_and_disjoint_from_added_sets() {
        let automation_legacy = AUTOMATION_RUNS_LEGACY_COLUMNS
            .into_iter()
            .collect::<BTreeSet<_>>();
        let automation_added = AUTOMATION_RUNS_ADDED_COLUMNS
            .into_iter()
            .map(|(name, _)| name)
            .collect::<BTreeSet<_>>();
        assert!(automation_legacy.is_disjoint(&automation_added));

        let mutation_legacy = LOCAL_MUTATIONS_LEGACY_COLUMNS
            .into_iter()
            .collect::<BTreeSet<_>>();
        let mutation_added = LOCAL_MUTATIONS_ADDED_COLUMNS
            .into_iter()
            .map(|(name, _)| name)
            .collect::<BTreeSet<_>>();
        assert!(mutation_legacy.is_disjoint(&mutation_added));
    }
}
