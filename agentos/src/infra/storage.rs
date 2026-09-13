//! 既存インストールのデータ保存先を選ぶアプリ側のポリシー。
//! パスは今回変更しない。SQLite アダプターには明示的に渡す。
use anyhow::{Context, Result};
use lineage_core::infra::sqlite::Database;
use std::path::PathBuf;
const DATABASE_FILE_NAME: &str = "lineage.db";

/// 既定のデータディレクトリ（`%LOCALAPPDATA%\minos`）の DB を開く。
pub fn open_default() -> Result<Database> {
    let path = default_path()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("データディレクトリを作成できません: {}", parent.display()))?;
    }
    Database::open(&path)
}

pub fn default_path() -> Result<PathBuf> {
    let dir = dirs::data_local_dir()
        .context("ローカルアプリケーションデータのディレクトリを特定できません")?;
    Ok(dir.join("minos").join(DATABASE_FILE_NAME))
}
