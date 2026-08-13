//! `InferenceBackend` port の実装（API キー方式）。
//!
//! Rust には公式の Anthropic SDK が無いので Messages API を直接叩く。
//! リクエストの組み立てで気をつける点:
//!
//! - `temperature` / `top_p` / `top_k` は送らない（現行モデルでは 400 になる）
//! - 思考の深さは `budget_tokens` ではなく `output_config.effort` で指示する
//! - `max_tokens` は「思考＋本文」の合計に効くので、要約用途でも余裕を持たせる
//! - 応答の `content` には thinking ブロックが先に来るので、先頭要素を本文とみなさない

use std::time::Duration;

use anyhow::{Context, Result, bail};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::domain::automation::{InferenceOutcome, InferenceRequest};
use crate::domain::ports::{CredentialStore, InferenceBackend};

/// この実装が扱える provider。
pub const PROVIDER: &str = "anthropic";

const ENDPOINT: &str = "https://api.anthropic.com/v1/messages";
const API_VERSION: &str = "2023-06-01";

/// 既定のモデル。ルール側で `backend_config.model` を指定すれば上書きできる。
pub const DEFAULT_MODEL: &str = "claude-opus-5";

/// 出力の上限。思考ぶんもここに含まれるので、要約用途でも切り詰めすぎない。
const MAX_TOKENS: u32 = 16_000;

/// 応答待ちの上限。思考を伴うと数分かかることがあるので長めにとる。
const TIMEOUT: Duration = Duration::from_secs(300);

/// Anthropic Messages API を呼ぶバックエンド。
pub struct AnthropicBackend<'a> {
    credentials: &'a dyn CredentialStore,
    endpoint: String,
}

impl<'a> AnthropicBackend<'a> {
    pub fn new(credentials: &'a dyn CredentialStore) -> Self {
        Self {
            credentials,
            endpoint: ENDPOINT.to_string(),
        }
    }

    /// 送信先を差し替える（テストでモックサーバへ向けるため）。
    #[cfg(any(test, feature = "testing"))]
    pub fn with_endpoint(mut self, endpoint: impl Into<String>) -> Self {
        self.endpoint = endpoint.into();
        self
    }
}

impl InferenceBackend for AnthropicBackend<'_> {
    fn complete(&self, request: &InferenceRequest) -> Result<InferenceOutcome> {
        if request.provider != PROVIDER {
            bail!(
                "provider `{}` には未対応です（対応しているのは `{PROVIDER}` のみ）",
                request.provider
            );
        }

        let api_key = self
            .credentials
            .secret(PROVIDER)?
            .with_context(|| format!("{PROVIDER} の API キーが未登録です。設定画面で登録してください"))?;

        let mut body = json!({
            "model": request.model.as_deref().unwrap_or(DEFAULT_MODEL),
            "max_tokens": MAX_TOKENS,
            // 思考の要否と深さはモデルに委ねる。固定のトークン予算は現行モデルでは使えない。
            "thinking": { "type": "adaptive" },
            "messages": [{ "role": "user", "content": request.prompt }],
        });
        if let Some(effort) = &request.effort {
            body["output_config"] = json!({ "effort": effort });
        }

        let client = reqwest::blocking::Client::builder()
            .timeout(TIMEOUT)
            .build()
            .context("HTTP クライアントを初期化できません")?;

        let response = client
            .post(&self.endpoint)
            .header("x-api-key", api_key)
            .header("anthropic-version", API_VERSION)
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .context("Anthropic API に接続できません")?;

        let status = response.status();
        let text = response.text().context("応答を読み取れません")?;
        if !status.is_success() {
            bail!("Anthropic API がエラーを返しました ({status}): {}", summarize(&text));
        }

        let message: MessageResponse =
            serde_json::from_str(&text).context("応答を解釈できません")?;
        Ok(message.into_outcome())
    }
}

/// 応答のうち、この実装が見る部分だけ。
#[derive(Deserialize)]
struct MessageResponse {
    #[serde(default)]
    content: Vec<ContentBlock>,
    #[serde(default)]
    stop_reason: Option<String>,
    #[serde(default)]
    stop_details: Option<StopDetails>,
}

#[derive(Deserialize)]
struct ContentBlock {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    text: Option<String>,
}

#[derive(Deserialize)]
struct StopDetails {
    #[serde(default)]
    category: Option<Value>,
}

impl MessageResponse {
    fn into_outcome(self) -> InferenceOutcome {
        // 拒否は content を読む前に判定する。拒否時の content は空か途中までしかない。
        if self.stop_reason.as_deref() == Some("refusal") {
            let category = self
                .stop_details
                .and_then(|details| details.category)
                .and_then(|value| value.as_str().map(str::to_string));
            return InferenceOutcome::Refused { category };
        }

        // thinking ブロックが先に来るので、text ブロックだけを拾って連結する。
        let text = self
            .content
            .into_iter()
            .filter(|block| block.kind == "text")
            .filter_map(|block| block.text)
            .collect::<Vec<_>>()
            .join("");
        InferenceOutcome::Completed(text)
    }
}

/// エラー本文が長くても、ログや画面が埋まらないように切り詰める。
fn summarize(body: &str) -> String {
    const LIMIT: usize = 500;
    let trimmed = body.trim();
    if trimmed.chars().count() <= LIMIT {
        return trimmed.to_string();
    }
    trimmed.chars().take(LIMIT).collect::<String>() + "…"
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(json: &str) -> InferenceOutcome {
        serde_json::from_str::<MessageResponse>(json)
            .unwrap()
            .into_outcome()
    }

    #[test]
    fn joins_only_the_text_blocks() {
        // thinking ブロックが先頭に来る形。content[0] を本文とみなすと壊れる。
        let outcome = parse(
            r#"{
                "stop_reason": "end_turn",
                "content": [
                    {"type": "thinking", "thinking": ""},
                    {"type": "text", "text": "前半"},
                    {"type": "text", "text": "後半"}
                ]
            }"#,
        );
        assert_eq!(outcome, InferenceOutcome::Completed("前半後半".into()));
    }

    #[test]
    fn a_refusal_is_not_treated_as_an_empty_answer() {
        let outcome = parse(
            r#"{
                "stop_reason": "refusal",
                "stop_details": {"type": "refusal", "category": "cyber"},
                "content": []
            }"#,
        );
        assert_eq!(
            outcome,
            InferenceOutcome::Refused {
                category: Some("cyber".into())
            }
        );
    }

    #[test]
    fn a_refusal_without_a_category_still_parses() {
        let outcome = parse(r#"{"stop_reason": "refusal", "stop_details": {"category": null}}"#);
        assert_eq!(outcome, InferenceOutcome::Refused { category: None });
    }

    #[test]
    fn an_unknown_provider_is_rejected_before_touching_the_network() {
        let credentials = crate::infra::credentials::StubCredentialStore(Some("k".into()));
        let backend = AnthropicBackend::new(&credentials);
        let request = InferenceRequest {
            provider: "openai".into(),
            prompt: "hi".into(),
            model: None,
            effort: None,
        };
        assert!(backend.complete(&request).is_err());
    }

    #[test]
    fn a_missing_api_key_is_reported_as_such() {
        let credentials = crate::infra::credentials::StubCredentialStore(None);
        let backend = AnthropicBackend::new(&credentials);
        let request = InferenceRequest {
            provider: PROVIDER.into(),
            prompt: "hi".into(),
            model: None,
            effort: None,
        };
        let error = backend.complete(&request).unwrap_err();
        assert!(format!("{error:#}").contains("未登録"));
    }

    #[test]
    fn long_error_bodies_are_truncated() {
        assert_eq!(summarize("  短い  "), "短い");
        assert_eq!(summarize(&"あ".repeat(600)).chars().count(), 501);
    }
}
