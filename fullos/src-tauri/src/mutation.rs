//! Rust 側でローカル DB への差分更新を確定する Tauri コマンド。
//!
//! FullOS の WebView に SQL の `execute` 権限を与えず、更新リクエストを同梱の
//! agentos へ標準入力で渡す。実際の DB 更新とドメイン検証は agentos から呼ばれる
//! lineage-core の application service が行う。

use std::sync::Mutex;

use serde_json::Value;
use tauri::AppHandle;

/// FullOS 内から同時に複数の writer sidecar を起動しない。
///
/// lineage-core 側も `BEGIN IMMEDIATE` と busy timeout で別プロセスとの競合を待つが、
/// 同一 UI 内の連打や終了時処理はここで先に直列化して余計なプロセスを増やさない。
static LOCAL_MUTATION_LOCK: Mutex<()> = Mutex::new(());

/// ローカルの差分更新を適用する。
///
/// `request` は `lineage_core::domain::mutation::MutationRequest` と同じ JSON 契約を
/// 使う。JSON の形を Tauri 側で複製して検証せず、Rust の共有 application service を
/// 唯一の検証・書き込み経路にする。
#[tauri::command]
pub async fn local_mutation_apply(
    app: AppHandle,
    request: Value,
) -> Result<Value, String> {
    let request = serde_json::to_string(&request)
        .map_err(|error| format!("差分更新リクエストを JSON に変換できません: {error}"))?;

    tauri::async_runtime::spawn_blocking(move || {
        let _guard = LOCAL_MUTATION_LOCK
            .lock()
            .map_err(|_| "ローカル差分更新のロックが壊れています".to_string())?;
        crate::automation::invoke_json(
            &app,
            &["--json", "apply", "--request-file", "-"],
            Some(&request),
        )
    })
    .await
    .map_err(|error| format!("差分更新を実行できません: {error}"))?
}
