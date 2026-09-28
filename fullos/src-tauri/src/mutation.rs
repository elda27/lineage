//! FullOS内で完結するローカルDBアクセス。自動化Runnerは起動しない。
use lineage_core::domain::mutation::MutationRequest;
use lineage_store::{
    features::mutation::ApplyMutation,
    infra::{clock::SystemClock, sqlite::Database},
};
use serde_json::Value;

fn database() -> Result<Database, String> {
    let directory = dirs::data_local_dir()
        .ok_or("ローカルデータディレクトリがありません")?
        .join("minos");
    std::fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
    Database::open(&directory.join("lineage.db")).map_err(|e| format!("{e:#}"))
}

#[tauri::command]
pub async fn local_mutation_apply(request: MutationRequest) -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let db = database()?;
        let result = ApplyMutation {
            store: &db,
            clock: &SystemClock,
        }
        .execute(request)
        .map_err(|e| format!("{e:#}"))?;
        serde_json::to_value(result).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn local_query(query: String, bind_values: Vec<Value>) -> Result<Vec<Value>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        database()?
            .select_json(&query, &bind_values)
            .map_err(|e| format!("{e:#}"))
    })
    .await
    .map_err(|e| e.to_string())?
}
