use super::inference::{InferenceOutcome, InferenceRequest};
use anyhow::Result;

/// API キーなどの秘密の取り出し。
///
/// 保存先は OS の資格情報ストア。`SQLite` には置かない（DB ファイルを読めた者が
/// そのまま鍵を持ち出せてしまうため）。
pub trait CredentialStore {
    /// 登録されていなければ `None`。呼び出し側が「未登録」を利用者に案内できるよう、
    /// 見つからないことをエラーにはしない。
    fn secret(&self, provider: &str) -> Result<Option<String>>;
}

/// 生成AIの呼び出し。
pub trait InferenceBackend {
    fn complete(&self, request: &InferenceRequest) -> Result<InferenceOutcome>;
}
