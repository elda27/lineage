//! minos の診断ログを、コンソールの有無にかかわらずファイルへ残す。

use std::fs::{File, OpenOptions};
use std::io;
use std::path::PathBuf;

use anyhow::{Context, Result};
use env_logger::{Builder, Env, Target};

use lineage_core::infra::sqlite::Database;

const LOG_FILE_NAME: &str = "minos.log";

/// `%LOCALAPPDATA%\minos\minos.log` へ追記するロガーを初期化する。
///
/// リリース版は Windows のコンソールを持たないため、標準エラーだけに出すと
/// 起動途中の失敗を調査できない。DB と同じ利用者別ディレクトリなら、インストール先への
/// 書き込み権限を必要とせず、利用者もログを見つけやすい。
pub fn init() -> Result<PathBuf> {
    let path = log_file_path()?;
    let parent = path.parent().context("ログディレクトリを特定できません")?;
    std::fs::create_dir_all(parent)
        .with_context(|| format!("ログディレクトリを作成できません: {}", parent.display()))?;
    let file = open_log_file(&path)
        .with_context(|| format!("ログファイルを開けません: {}", path.display()))?;

    Builder::from_env(Env::default().default_filter_or("info"))
        .target(Target::Pipe(Box::new(file)))
        .format_timestamp_millis()
        .try_init()
        .context("ロガーを初期化できません")?;

    install_panic_hook();
    Ok(path)
}

fn log_file_path() -> Result<PathBuf> {
    let database = Database::default_path()?;
    let directory = database
        .parent()
        .context("データディレクトリを特定できません")?;
    Ok(directory.join(LOG_FILE_NAME))
}

fn open_log_file(path: &std::path::Path) -> io::Result<File> {
    OpenOptions::new().create(true).append(true).open(path)
}

fn install_panic_hook() {
    std::panic::set_hook(Box::new(|panic| {
        let location = panic
            .location()
            .map(|location| {
                format!(
                    "{}:{}:{}",
                    location.file(),
                    location.line(),
                    location.column()
                )
            })
            .unwrap_or_else(|| "不明".into());
        let message = panic
            .payload()
            .downcast_ref::<&str>()
            .copied()
            .or_else(|| panic.payload().downcast_ref::<String>().map(String::as_str))
            .unwrap_or("メッセージなし");
        log::error!(
            "panic が発生しました: location={location}, message={message}\nbacktrace={}",
            std::backtrace::Backtrace::force_capture()
        );
    }));
}
