//! 自動化の定期実行を OS のスケジューラに登録する。
//!
//! agentos は常駐しない（`agentos --help` の説明どおり、状態も持たない）。
//! そのため「メタ情報マッチ」と「スケジュール」のルールを動かすには、誰かが定期的に
//! `agentos tick` を起動する必要がある。その役をここで OS のスケジューラに任せる。
//!
//! 登録は利用者が設定画面で明示的に有効にしたときだけ行う。アプリが黙って OS に
//! タスクを作るのは、利用者にとって説明のつかない変化になるため。

use std::process::{Command, Stdio};

use tauri::AppHandle;

/// 登録するタスクの名前。利用者がタスクスケジューラで見つけられる名前にする。
const TASK_NAME: &str = "lineage-automation";

/// 定期実行の間隔（分）。
///
/// 短すぎると起動のたびに DB を開いて無駄が出る。長すぎるとメタ情報マッチの
/// 反応が鈍く感じられる。その折り合いとして15分にしてある。
const INTERVAL_MINUTES: u32 = 15;

/// タスクが登録済みかどうか。
#[tauri::command]
pub async fn schedule_status() -> Result<bool, String> {
    if !cfg!(windows) {
        return Ok(false);
    }
    tauri::async_runtime::spawn_blocking(|| {
        // /Query は未登録だと非ゼロで終わる。エラー本文は見ずに終了コードだけで判断する。
        let status = schtasks(&["/Query", "/TN", TASK_NAME])?;
        Ok(status.0)
    })
    .await
    .map_err(|error| format!("処理を実行できません: {error}"))?
}

/// 定期実行を登録する（既にあれば作り直す）。
#[tauri::command]
pub async fn schedule_register(app: AppHandle) -> Result<(), String> {
    if !cfg!(windows) {
        return Err("定期実行の自動登録は現在 Windows のみ対応しています".to_string());
    }

    let agentos = crate::automation::agentos_path(&app)?;
    let command = format!("\"{}\" tick", agentos.display());

    tauri::async_runtime::spawn_blocking(move || {
        let (ok, output) = schtasks(&[
            "/Create",
            "/TN",
            TASK_NAME,
            "/TR",
            &command,
            "/SC",
            "MINUTE",
            "/MO",
            &INTERVAL_MINUTES.to_string(),
            // 既存があれば置き換える。間隔や agentos の場所が変わったときに更新できる。
            "/F",
        ])?;
        if ok {
            Ok(())
        } else {
            Err(format!("定期実行を登録できませんでした: {output}"))
        }
    })
    .await
    .map_err(|error| format!("処理を実行できません: {error}"))?
}

/// 定期実行の登録を解除する。
#[tauri::command]
pub async fn schedule_unregister() -> Result<(), String> {
    if !cfg!(windows) {
        return Ok(());
    }
    tauri::async_runtime::spawn_blocking(|| {
        let (ok, output) = schtasks(&["/Delete", "/TN", TASK_NAME, "/F"])?;
        // 未登録の解除は成功と同じ結果なので、エラーにしない。
        if ok || output.contains("ERROR: The system cannot find the file specified") {
            Ok(())
        } else {
            Err(format!("定期実行を解除できませんでした: {output}"))
        }
    })
    .await
    .map_err(|error| format!("処理を実行できません: {error}"))?
}

/// `schtasks` を1回叩き、(成功したか, 出力) を返す。
fn schtasks(args: &[&str]) -> Result<(bool, String), String> {
    let mut command = Command::new("schtasks");
    command.args(args).stdout(Stdio::piped()).stderr(Stdio::piped());

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }

    let output = command
        .output()
        .map_err(|error| format!("schtasks を実行できません: {error}"))?;

    let mut text = String::from_utf8_lossy(&output.stdout).into_owned();
    text.push_str(&String::from_utf8_lossy(&output.stderr));
    Ok((output.status.success(), text.trim().to_string()))
}
