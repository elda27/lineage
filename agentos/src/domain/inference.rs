use serde::{Deserialize, Serialize};

/// 生成AIに投げる1回ぶんの依頼。
///
/// バックエンドの差（HTTP かブラウザか）を application より内側に持ち込まないための形。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InferenceRequest {
    pub provider: String,
    pub prompt: String,
    pub model: Option<String>,
    pub effort: Option<String>,
}

/// 生成AIからの応答。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InferenceOutcome {
    /// 本文が返った。
    Completed(String),
    /// 安全上の理由で応答が拒否された。
    ///
    /// 通信失敗と分けているのは、再試行しても結果が変わらないため。利用者には
    /// 「プロンプトを見直す」という別の行動を促したい。
    Refused { category: Option<String> },
}
