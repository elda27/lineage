//! fullos の起動。
//!
//! fullos は minos と同じディレクトリに置かれる想定（インストーラが両方を配置する）。
//! 開発中はまだ存在しないため、見つからないことを異常終了ではなくメッセージとして返す。

use std::path::PathBuf;
use std::process::Command;

use anyhow::{Context, Result, bail};

#[cfg(windows)]
const FULLOS_EXECUTABLE: &str = "fullos.exe";
#[cfg(not(windows))]
const FULLOS_EXECUTABLE: &str = "fullos";

/// fullos を起動する。起動できた場合はそのパスを返す。
pub fn launch_fullos() -> Result<PathBuf> {
    let path = fullos_path()?;
    if !path.exists() {
        bail!("fullos が見つかりません: {}", path.display());
    }

    Command::new(&path)
        .spawn()
        .with_context(|| format!("fullos を起動できません: {}", path.display()))?;
    Ok(path)
}

fn fullos_path() -> Result<PathBuf> {
    let exe = std::env::current_exe().context("実行ファイルのパスを取得できません")?;
    let dir = exe
        .parent()
        .context("実行ファイルのディレクトリを取得できません")?;
    Ok(dir.join(FULLOS_EXECUTABLE))
}
