//! `CredentialStore` port の実装。OS の資格情報ストアを使う。
//!
//! API キーを `settings` テーブルに置かないのは、`lineage.db` を読める者が
//! そのまま鍵を持ち出せてしまうため。DB には参照キーすら書かない
//! （どの provider が登録済みかは、ストアに問い合わせれば分かる）。
//!
//! 保存先は Windows なら資格情報マネージャー、macOS ならキーチェーン、
//! Linux なら Secret Service。

use anyhow::{Context, Result};

use crate::domain::ports::CredentialStore;

/// 資格情報ストア上のサービス名。利用者にはこの名前で見える。
const SERVICE: &str = "lineage";

/// account 名の接頭辞。将来ほかの用途の秘密を置いても衝突しないようにする。
const ACCOUNT_PREFIX: &str = "automation/";

/// OS の資格情報ストア。
pub struct OsCredentialStore;

impl OsCredentialStore {
    fn entry(provider: &str) -> Result<keyring::Entry> {
        let account = format!("{ACCOUNT_PREFIX}{provider}");
        keyring::Entry::new(SERVICE, &account)
            .with_context(|| format!("資格情報ストアを開けません: {account}"))
    }

    /// 秘密を保存する（既存があれば上書き）。
    pub fn set(provider: &str, secret: &str) -> Result<()> {
        Self::entry(provider)?
            .set_password(secret)
            .with_context(|| format!("{provider} の資格情報を保存できません"))
    }

    /// 登録済みかどうか。値そのものは返さない。
    ///
    /// 設定画面は「登録済み / 未登録」だけ出せればよく、平文を webview に渡す理由がない。
    pub fn has(provider: &str) -> Result<bool> {
        Ok(Self.secret(provider)?.is_some())
    }

    /// 登録を削除する。未登録でもエラーにしない（削除の結果としては同じため）。
    pub fn delete(provider: &str) -> Result<()> {
        match Self::entry(provider)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(error) => {
                Err(anyhow::Error::new(error).context(format!("{provider} の資格情報を削除できません")))
            }
        }
    }
}

impl CredentialStore for OsCredentialStore {
    fn secret(&self, provider: &str) -> Result<Option<String>> {
        match Self::entry(provider)?.get_password() {
            Ok(secret) => Ok(Some(secret)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(error) => {
                Err(anyhow::Error::new(error).context(format!("{provider} の資格情報を読めません")))
            }
        }
    }
}

/// テスト用の、メモリ上に置くだけの資格情報ストア。
#[cfg(any(test, feature = "testing"))]
pub struct StubCredentialStore(pub Option<String>);

#[cfg(any(test, feature = "testing"))]
impl CredentialStore for StubCredentialStore {
    fn secret(&self, _provider: &str) -> Result<Option<String>> {
        Ok(self.0.clone())
    }
}
