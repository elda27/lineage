//! 自動化の Tauri コマンド。
//!
//! 実行そのものは同梱の `agentos.exe` に任せ、ここはその呼び出しに徹する。
//!
//! agentos を別プロセスとして呼ぶ理由は2つある。
//!
//! 1. `tauri-plugin-sql`(sqlx) と `rusqlite` はどちらも native の sqlite3 を
//!    リンクするため、同じバイナリには同居できない。
//! 2. 仮に同居できたとしても、1プロセスに sqlite の接続スタックが2つある状態
//!    （webview 側の読み出しと Rust 側の書き込み）は避けたい。
//!
//! 結果として、`links` への追記は minos / agentos の1本に集約されたままになる。
//! fullos は「どのルールをどの記録に当てるか」を決めるだけで、鎖には直接触らない。
//!
//! API キーの平文は webview に渡さない。公開するのは登録・削除・有無の確認だけで、
//! 値を読み出すコマンドは用意していない。登録時の値も引数ではなく標準入力で渡す
//! （コマンドライン引数は他プロセスから見えるため）。

use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use serde_json::Value;
use tauri::{AppHandle, Manager};

/// 同梱している実行ファイルの名前。
const AGENTOS_EXE: &str = if cfg!(windows) { "agentos.exe" } else { "agentos" };

/// agentos の実行ファイルを探す。
///
/// 配布時は Tauri のリソースディレクトリに入る（tauri.conf.json の bundle.resources）。
/// 開発時はワークスペースの target/ に出るので、そちらも見に行く。
pub fn agentos_path(app: &AppHandle) -> Result<PathBuf, String> {
    if let Ok(resource_dir) = app.path().resource_dir() {
        let bundled = resource_dir.join(AGENTOS_EXE);
        if bundled.exists() {
            return Ok(bundled);
        }
    }

    // 開発時: src-tauri/target/... から見てワークスペースルートの target/ を辿る。
    let exe = std::env::current_exe().map_err(|error| format!("実行ファイルの場所を特定できません: {error}"))?;
    for ancestor in exe.ancestors() {
        for profile in ["debug", "release"] {
            let candidate = ancestor.join("target").join(profile).join(AGENTOS_EXE);
            if candidate.exists() {
                return Ok(candidate);
            }
        }
    }

    Err(format!(
        "{AGENTOS_EXE} が見つかりません。`cargo build -p agentos` を実行してください"
    ))
}

/// agentos を1回呼び、stdout を返す。
///
/// 終了コード 2 は「自動化が成功しなかった」ことを表し、実行そのものの失敗（1）とは
/// 区別する。前者は結果 JSON が stdout に出ているので、呼び出し側に渡して判断させる。
fn invoke(app: &AppHandle, args: &[&str], stdin: Option<&str>) -> Result<String, String> {
    let path = agentos_path(app)?;
    let mut command = Command::new(&path);
    command
        .args(args)
        .stdin(if stdin.is_some() { Stdio::piped() } else { Stdio::null() })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    // コンソールウィンドウを出さない（fullos は GUI アプリなので、黒い窓が一瞬光る）。
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }

    let mut child = command
        .spawn()
        .map_err(|error| format!("{} を起動できません: {error}", path.display()))?;

    if let Some(text) = stdin {
        child
            .stdin
            .as_mut()
            .ok_or_else(|| "標準入力を開けません".to_string())?
            .write_all(text.as_bytes())
            .map_err(|error| format!("標準入力に書き込めません: {error}"))?;
        // drop して EOF を送らないと、agentos が read_to_string で待ち続ける。
        drop(child.stdin.take());
    }

    let output = child
        .wait_with_output()
        .map_err(|error| format!("agentos の終了を待てません: {error}"))?;

    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    match output.status.code() {
        // 0 = 成功、2 = 自動化が成功しなかった（結果は stdout にある）。
        Some(0) | Some(2) => Ok(stdout),
        _ => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let message = stderr.trim();
            Err(if message.is_empty() {
                "agentos の実行に失敗しました".to_string()
            } else {
                message.to_string()
            })
        }
    }
}

/// stdout の JSON を値として返す。
fn invoke_json(app: &AppHandle, args: &[&str], stdin: Option<&str>) -> Result<Value, String> {
    let stdout = invoke(app, args, stdin)?;
    serde_json::from_str(&stdout)
        .map_err(|error| format!("agentos の出力を解釈できません: {error}\n{stdout}"))
}

/// 記録に対して実行できるルール。メモの隣の「Action」ボタンが使う。
#[tauri::command]
pub async fn automation_match(app: AppHandle, memo_id: String) -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(move || {
        invoke_json(&app, &["--json", "match", "--memo", &memo_id], None)
    })
    .await
    .map_err(|error| format!("処理を実行できません: {error}"))?
}

/// API キー方式で1件実行する。
///
/// 生成AIの応答は数十秒〜数分かかるので、必ずブロッキングスレッドへ逃がす。
#[tauri::command]
pub async fn automation_run(
    app: AppHandle,
    rule_id: String,
    memo_id: String,
) -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(move || {
        invoke_json(
            &app,
            &["--json", "run", "--rule", &rule_id, "--memo", &memo_id],
            None,
        )
    })
    .await
    .map_err(|error| format!("処理を実行できません: {error}"))?
}

/// 送るプロンプトを組み立てる（ブラウザ方式の前段）。
#[tauri::command]
pub async fn automation_render(
    app: AppHandle,
    rule_id: String,
    memo_id: String,
) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let value = invoke_json(
            &app,
            &["--json", "render", "--rule", &rule_id, "--memo", &memo_id],
            None,
        )?;
        value["prompt"]
            .as_str()
            .map(str::to_string)
            .ok_or_else(|| "プロンプトを取得できません".to_string())
    })
    .await
    .map_err(|error| format!("処理を実行できません: {error}"))?
}

/// ブラウザで得た結果を確定する（ブラウザ方式の後段）。
#[tauri::command]
pub async fn automation_record(
    app: AppHandle,
    rule_id: String,
    memo_id: String,
    text: String,
) -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(move || {
        invoke_json(
            &app,
            &[
                "--json",
                "record",
                "--rule",
                &rule_id,
                "--memo",
                &memo_id,
                "--result-file",
                "-",
            ],
            Some(&text),
        )
    })
    .await
    .map_err(|error| format!("処理を実行できません: {error}"))?
}

/// API キーを登録する（既存があれば上書き）。値は標準入力で渡す。
#[tauri::command]
pub async fn credential_set(
    app: AppHandle,
    provider: String,
    secret: String,
) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        invoke(&app, &["credential", "set", "--provider", &provider], Some(&secret)).map(|_| ())
    })
    .await
    .map_err(|error| format!("処理を実行できません: {error}"))?
}

/// 登録済みかどうかだけを返す。値そのものは webview に渡さない。
#[tauri::command]
pub async fn credential_has(app: AppHandle, provider: String) -> Result<bool, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let value = invoke_json(
            &app,
            &["--json", "credential", "has", "--provider", &provider],
            None,
        )?;
        Ok(value["registered"].as_bool().unwrap_or(false))
    })
    .await
    .map_err(|error| format!("処理を実行できません: {error}"))?
}

/// 登録を削除する。
#[tauri::command]
pub async fn credential_delete(app: AppHandle, provider: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        invoke(&app, &["credential", "delete", "--provider", &provider], None).map(|_| ())
    })
    .await
    .map_err(|error| format!("処理を実行できません: {error}"))?
}

/// hash-chain の検証。自動化が鎖を壊していないことを設定画面から確認できる。
#[tauri::command]
pub async fn verify_lineage(app: AppHandle) -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(move || invoke_json(&app, &["--json", "verify"], None))
        .await
        .map_err(|error| format!("処理を実行できません: {error}"))?
}
