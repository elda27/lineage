//! fullos からのローカル DB mutation。
//!
//! WebView には `sql:default`（load/select/close）のみを許可し、INSERT / UPDATE /
//! DELETE は Tauri command を経由して Rust 側で実行する。これは SQL injection 対策を
//! 主目的とした境界ではなく、書き込みを application boundary に集約して不変条件を
//! 迂回する経路を作らないための境界である。

use std::time::Duration;

use serde::Deserialize;
use serde_json::Value;
use sqlx::sqlite::{SqliteConnectOptions, SqliteConnection};
use sqlx::{Connection, QueryBuilder, Sqlite};
use tauri::{AppHandle, Manager};

const MINOS_DIRECTORY: &str = "minos";
const DATABASE_FILE_NAME: &str = "lineage.db";

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutomationRulePayload {
    id: String,
    workspace_id: String,
    name: String,
    description: Option<String>,
    prompt: String,
    backend: String,
    backend_config: Value,
    trigger_kind: String,
    trigger: Value,
    enabled: bool,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TagUpdatePayload {
    display_name: String,
    shorthand: Option<String>,
    enabled: bool,
    view: Option<String>,
    recipe: Option<String>,
    recipe_managed: bool,
}

async fn open_database(app: &AppHandle) -> Result<SqliteConnection, String> {
    let path = app
        .path()
        .local_data_dir()
        .map_err(|error| format!("ローカルデータディレクトリを特定できません: {error}"))?
        .join(MINOS_DIRECTORY)
        .join(DATABASE_FILE_NAME);

    let options = SqliteConnectOptions::new()
        .filename(&path)
        .create_if_missing(false)
        .busy_timeout(Duration::from_secs(3));

    SqliteConnection::connect_with(&options)
        .await
        .map_err(|error| format!("データベースを開けません ({}): {error}", path.display()))
}

#[tauri::command]
pub async fn memo_set_done(
    app: AppHandle,
    workspace_id: String,
    memo_id: String,
    done: bool,
    at: String,
) -> Result<(), String> {
    let mut db = open_database(&app).await?;
    sqlx::query(
        "INSERT INTO document_states (document_id, workspace_id, done, done_at, updated_at)\n         VALUES (?, ?, ?, ?, ?)\n         ON CONFLICT(document_id) DO UPDATE\n           SET done = excluded.done, done_at = excluded.done_at, updated_at = excluded.updated_at",
    )
    .bind(memo_id)
    .bind(workspace_id)
    .bind(if done { 1 } else { 0 })
    .bind(if done { Some(at.as_str()) } else { None })
    .bind(&at)
    .execute(&mut db)
    .await
    .map_err(write_error)?;
    Ok(())
}

#[tauri::command]
pub async fn memo_set_archived(
    app: AppHandle,
    workspace_id: String,
    memo_id: String,
    archived_at: Option<String>,
    updated_at: String,
) -> Result<(), String> {
    let mut db = open_database(&app).await?;
    sqlx::query(
        "INSERT INTO document_states (document_id, workspace_id, archived_at, updated_at)\n         VALUES (?, ?, ?, ?)\n         ON CONFLICT(document_id) DO UPDATE\n           SET archived_at = excluded.archived_at, updated_at = excluded.updated_at",
    )
    .bind(memo_id)
    .bind(workspace_id)
    .bind(archived_at)
    .bind(updated_at)
    .execute(&mut db)
    .await
    .map_err(write_error)?;
    Ok(())
}

#[tauri::command]
pub async fn memo_trash(
    app: AppHandle,
    workspace_id: String,
    memo_id: String,
    at: String,
) -> Result<(), String> {
    let mut db = open_database(&app).await?;
    sqlx::query(
        "INSERT INTO document_states (document_id, workspace_id, deleted_at, updated_at)\n         VALUES (?, ?, ?, ?)\n         ON CONFLICT(document_id) DO UPDATE\n           SET deleted_at = excluded.deleted_at, updated_at = excluded.updated_at",
    )
    .bind(memo_id)
    .bind(workspace_id)
    .bind(&at)
    .bind(&at)
    .execute(&mut db)
    .await
    .map_err(write_error)?;
    Ok(())
}

#[tauri::command]
pub async fn memo_archive_done(
    app: AppHandle,
    workspace_id: String,
    labels: Vec<String>,
    at: String,
) -> Result<(), String> {
    if labels.is_empty() {
        return Ok(());
    }

    let mut db = open_database(&app).await?;
    let mut query = QueryBuilder::<Sqlite>::new(
        "UPDATE document_states SET archived_at = ",
    );
    query
        .push_bind(&at)
        .push(", updated_at = ")
        .push_bind(&at)
        .push(" WHERE workspace_id = ")
        .push_bind(&workspace_id)
        .push(
            " AND done = 1 AND archived_at IS NULL AND deleted_at IS NULL\n             AND document_id IN (SELECT document_id FROM document_meta WHERE lower(label) IN (",
        );
    {
        let mut separated = query.separated(", ");
        for label in labels {
            separated.push_bind(label.to_lowercase());
        }
    }
    query.push("))");
    query
        .build()
        .execute(&mut db)
        .await
        .map_err(write_error)?;
    Ok(())
}

#[tauri::command]
pub async fn automation_rule_save(
    app: AppHandle,
    rule: AutomationRulePayload,
) -> Result<(), String> {
    let mut db = open_database(&app).await?;
    let backend_config = serde_json::to_string(&rule.backend_config)
        .map_err(|error| format!("backend_config を保存できません: {error}"))?;
    let trigger_config = serde_json::to_string(&rule.trigger)
        .map_err(|error| format!("trigger_config を保存できません: {error}"))?;

    sqlx::query(
        "INSERT INTO automation_rules\n           (id, workspace_id, name, description, prompt, backend_kind, backend_config,\n            trigger_kind, trigger_config, enabled, created_at, updated_at)\n         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)\n         ON CONFLICT(id) DO UPDATE SET\n           name = excluded.name, description = excluded.description, prompt = excluded.prompt,\n           backend_kind = excluded.backend_kind, backend_config = excluded.backend_config,\n           trigger_kind = excluded.trigger_kind, trigger_config = excluded.trigger_config,\n           enabled = excluded.enabled, updated_at = excluded.updated_at",
    )
    .bind(rule.id)
    .bind(rule.workspace_id)
    .bind(rule.name)
    .bind(rule.description)
    .bind(rule.prompt)
    .bind(rule.backend)
    .bind(backend_config)
    .bind(rule.trigger_kind)
    .bind(trigger_config)
    .bind(if rule.enabled { 1 } else { 0 })
    .bind(rule.created_at)
    .bind(rule.updated_at)
    .execute(&mut db)
    .await
    .map_err(write_error)?;
    Ok(())
}

#[tauri::command]
pub async fn automation_rule_delete(app: AppHandle, id: String) -> Result<(), String> {
    let mut db = open_database(&app).await?;
    sqlx::query("DELETE FROM automation_rules WHERE id = ?")
        .bind(id)
        .execute(&mut db)
        .await
        .map_err(write_error)?;
    Ok(())
}

#[tauri::command]
pub async fn setting_set(
    app: AppHandle,
    workspace_id: String,
    key: String,
    value: String,
    at: String,
) -> Result<(), String> {
    let mut db = open_database(&app).await?;
    sqlx::query(
        "INSERT INTO settings (workspace_id, key, value, updated_at) VALUES (?, ?, ?, ?)\n         ON CONFLICT(workspace_id, key)\n         DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
    )
    .bind(workspace_id)
    .bind(key)
    .bind(value)
    .bind(at)
    .execute(&mut db)
    .await
    .map_err(write_error)?;
    Ok(())
}

#[tauri::command]
pub async fn tag_update(
    app: AppHandle,
    id: String,
    value: TagUpdatePayload,
    at: String,
) -> Result<(), String> {
    let mut db = open_database(&app).await?;
    let mut tx = db.begin().await.map_err(write_error)?;

    sqlx::query(
        "UPDATE tag_definitions SET display_name = ?, shorthand = ?, enabled = ?, updated_at = ? WHERE id = ?",
    )
    .bind(value.display_name)
    .bind(value.shorthand)
    .bind(if value.enabled { 1 } else { 0 })
    .bind(&at)
    .bind(&id)
    .execute(&mut *tx)
    .await
    .map_err(write_error)?;

    sqlx::query("DELETE FROM view_bindings WHERE tag_id = ?")
        .bind(&id)
        .execute(&mut *tx)
        .await
        .map_err(write_error)?;
    if let Some(view) = value.view {
        sqlx::query("INSERT INTO view_bindings VALUES (?, ?, ?)")
            .bind(&id)
            .bind(view)
            .bind(&at)
            .execute(&mut *tx)
            .await
            .map_err(write_error)?;
    }

    sqlx::query("DELETE FROM automation_bindings WHERE tag_id = ?")
        .bind(&id)
        .execute(&mut *tx)
        .await
        .map_err(write_error)?;
    if let Some(recipe) = value.recipe {
        sqlx::query("INSERT INTO automation_bindings VALUES (?, ?, ?, ?, ?)")
            .bind(&id)
            .bind(recipe)
            .bind(if value.recipe_managed { "managed" } else { "external" })
            .bind(1)
            .bind(&at)
            .execute(&mut *tx)
            .await
            .map_err(write_error)?;
    }

    tx.commit().await.map_err(write_error)?;
    Ok(())
}

#[tauri::command]
pub async fn tag_delete(app: AppHandle, id: String, at: String) -> Result<(), String> {
    let mut db = open_database(&app).await?;
    sqlx::query(
        "UPDATE tag_definitions SET deleted_at = ?, enabled = 0, updated_at = ? WHERE id = ? AND kind = 'user'",
    )
    .bind(&at)
    .bind(&at)
    .bind(id)
    .execute(&mut db)
    .await
    .map_err(write_error)?;
    Ok(())
}

fn write_error(error: sqlx::Error) -> String {
    if error.to_string().contains("no such table") {
        "状態を保存できませんでした。minos を一度起動してデータベースを最新にしてください。"
            .to_string()
    } else {
        format!("データベースの更新に失敗しました: {error}")
    }
}
