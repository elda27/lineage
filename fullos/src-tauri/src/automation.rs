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
//! 通常のローカル更新も同じ agentos の writer 経由で確定する。
//!
//! API キーの平文は webview に渡さない。公開するのは登録・削除・有無の確認だけで、
//! 値を読み出すコマンドは用意していない。登録時の値も引数ではなく標準入力で渡す
//! （コマンドライン引数は他プロセスから見えるため）。

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde_json::Value;
use tauri::{AppHandle, Manager};

/// 同梱している実行ファイルの名前。
const AGENTOS_EXE: &str = if cfg!(windows) {
    "agentos.exe"
} else {
    "agentos"
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ExitCodePolicy {
    /// コマンドが正常終了した場合だけ stdout を受け取る。
    SuccessOnly,
    /// 終了コード 2 でも、機械可読な結果が stdout にある場合は受け取る。
    ReportedOutcome,
}

fn workspace_root(exe: &Path) -> Option<PathBuf> {
    exe.ancestors()
        .find(|ancestor| ancestor.join("agentos").join("Cargo.toml").is_file())
        .map(Path::to_path_buf)
}

fn agentos_candidates(
    workspace_root: Option<&Path>,
    resource_dir: Option<&Path>,
    debug_build: bool,
) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    match (debug_build, workspace_root) {
        // 開発中にワークスペースを特定できた場合は、古い resource copy へ
        // フォールバックしない。beforeDevCommand がこのファイルを必ずビルドする。
        (true, Some(root)) => {
            candidates.push(root.join("target").join("debug").join(AGENTOS_EXE));
        }
        (true, None) => {
            if let Some(resource_dir) = resource_dir {
                candidates.push(resource_dir.join(AGENTOS_EXE));
            }
        }
        (false, root) => {
            if let Some(resource_dir) = resource_dir {
                candidates.push(resource_dir.join(AGENTOS_EXE));
            }
            if let Some(root) = root {
                let candidate = root.join("target").join("release").join(AGENTOS_EXE);
                if !candidates.contains(&candidate) {
                    candidates.push(candidate);
                }
            }
        }
    }

    candidates
}

/// agentos の実行ファイルを探す。
///
/// 配布時は Tauri のリソースディレクトリに入る（tauri.conf.json の bundle.resources）。
/// 開発時はワークスペースの target/ に出るので、そちらも見に行く。
pub fn agentos_path(app: &AppHandle) -> Result<PathBuf, String> {
    let exe = std::env::current_exe()
        .map_err(|error| format!("実行ファイルの場所を特定できません: {error}"))?;
    let workspace_root = workspace_root(&exe);
    let resource_dir = app.path().resource_dir().ok();

    // Debug ではワークスペースの同じ profile を最優先にする。配布用 Release では
    // bundle.resources が配置した sidecar を最優先にする。
    for candidate in agentos_candidates(
        workspace_root.as_deref(),
        resource_dir.as_deref(),
        cfg!(debug_assertions),
    ) {
        if candidate.is_file() {
            return Ok(candidate);
        }
    }

    Err(format!(
        "{AGENTOS_EXE} が見つかりません。`cargo build -p agentos` を実行してください"
    ))
}

fn resolve_output(
    code: Option<i32>,
    stdout: String,
    stderr: &str,
    policy: ExitCodePolicy,
) -> Result<String, String> {
    if code == Some(0) {
        return Ok(stdout);
    }

    // Clap の引数エラーも終了コード 2 になる。結果 JSON がない場合まで成功扱いに
    // すると、stderr の本当の原因が隠れて空文字列の JSON parse error に化ける。
    if code == Some(2)
        && policy == ExitCodePolicy::ReportedOutcome
        && !stdout.trim().is_empty()
        && serde_json::from_str::<Value>(&stdout).is_ok()
    {
        return Ok(stdout);
    }

    let message = stderr.trim();
    Err(if message.is_empty() {
        match code {
            Some(code) => format!("agentos の実行に失敗しました (終了コード: {code})"),
            None => "agentos の実行に失敗しました (シグナルにより終了)".to_string(),
        }
    } else {
        message.to_string()
    })
}

/// agentos を1回呼び、stdout を返す。
fn invoke_with_policy(
    app: &AppHandle,
    args: &[&str],
    stdin: Option<&str>,
    policy: ExitCodePolicy,
) -> Result<String, String> {
    let path = agentos_path(app)?;
    let mut command = Command::new(&path);
    command
        .args(args)
        .stdin(if stdin.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
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

    resolve_output(
        output.status.code(),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        &String::from_utf8_lossy(&output.stderr),
        policy,
    )
}

fn invoke(app: &AppHandle, args: &[&str], stdin: Option<&str>) -> Result<String, String> {
    invoke_with_policy(app, args, stdin, ExitCodePolicy::SuccessOnly)
}

/// stdout の JSON を値として返す。
fn invoke_json_with_policy(
    app: &AppHandle,
    args: &[&str],
    stdin: Option<&str>,
    policy: ExitCodePolicy,
) -> Result<Value, String> {
    let stdout = invoke_with_policy(app, args, stdin, policy)?;
    if stdout.trim().is_empty() {
        return Err("agentos が JSON を出力しませんでした".to_string());
    }
    serde_json::from_str(&stdout)
        .map_err(|error| format!("agentos の出力を解釈できません: {error}\n{stdout}"))
}

pub(crate) fn invoke_json(
    app: &AppHandle,
    args: &[&str],
    stdin: Option<&str>,
) -> Result<Value, String> {
    invoke_json_with_policy(app, args, stdin, ExitCodePolicy::SuccessOnly)
}

fn invoke_reported_outcome_json(
    app: &AppHandle,
    args: &[&str],
    stdin: Option<&str>,
) -> Result<Value, String> {
    invoke_json_with_policy(app, args, stdin, ExitCodePolicy::ReportedOutcome)
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
        invoke_reported_outcome_json(
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
        invoke_reported_outcome_json(
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
        invoke(
            &app,
            &["credential", "set", "--provider", &provider],
            Some(&secret),
        )
        .map(|_| ())
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
        invoke(
            &app,
            &["credential", "delete", "--provider", &provider],
            None,
        )
        .map(|_| ())
    })
    .await
    .map_err(|error| format!("処理を実行できません: {error}"))?
}

/// hash-chain の検証。自動化が鎖を壊していないことを設定画面から確認できる。
#[tauri::command]
pub async fn verify_lineage(app: AppHandle) -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(move || {
        invoke_reported_outcome_json(&app, &["--json", "verify"], None)
    })
    .await
    .map_err(|error| format!("処理を実行できません: {error}"))?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_workspace_root_from_tauri_target() {
        let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        let expected = manifest_dir
            .parent()
            .and_then(Path::parent)
            .expect("src-tauri must be nested below the workspace root");
        let exe = manifest_dir.join("target").join("debug").join("fullos.exe");

        assert_eq!(workspace_root(&exe), Some(expected.to_path_buf()));
    }

    #[test]
    fn debug_prefers_workspace_debug_sidecar_over_bundled_copy() {
        let root = Path::new("workspace");
        let resources = Path::new("resources");

        assert_eq!(
            agentos_candidates(Some(root), Some(resources), true),
            vec![root.join("target").join("debug").join(AGENTOS_EXE)]
        );
    }

    #[test]
    fn release_prefers_bundled_sidecar_then_workspace_release() {
        let root = Path::new("workspace");
        let resources = Path::new("resources");

        assert_eq!(
            agentos_candidates(Some(root), Some(resources), false),
            vec![
                resources.join(AGENTOS_EXE),
                root.join("target").join("release").join(AGENTOS_EXE),
            ]
        );
    }

    #[test]
    fn reported_outcome_accepts_exit_two_with_json() {
        let stdout = r#"{"status":"failed"}"#.to_string();

        assert_eq!(
            resolve_output(Some(2), stdout.clone(), "", ExitCodePolicy::ReportedOutcome,),
            Ok(stdout)
        );
    }

    #[test]
    fn reported_outcome_rejects_exit_two_without_stdout() {
        let error = resolve_output(
            Some(2),
            String::new(),
            "error: unrecognized subcommand 'apply'",
            ExitCodePolicy::ReportedOutcome,
        )
        .expect_err("a clap parse failure must not be treated as a reported outcome");

        assert_eq!(error, "error: unrecognized subcommand 'apply'");
    }

    #[test]
    fn reported_outcome_rejects_exit_two_with_non_json_stdout() {
        let error = resolve_output(
            Some(2),
            "Usage: agentos apply".to_string(),
            "command line could not be parsed",
            ExitCodePolicy::ReportedOutcome,
        )
        .expect_err("only a JSON outcome may use the domain exit code");

        assert_eq!(error, "command line could not be parsed");
    }

    #[test]
    fn strict_commands_reject_exit_two_even_with_stdout() {
        let error = resolve_output(
            Some(2),
            r#"{"status":"failed"}"#.to_string(),
            "domain command failed",
            ExitCodePolicy::SuccessOnly,
        )
        .expect_err("strict commands must require exit code zero");

        assert_eq!(error, "domain command failed");
    }
}
