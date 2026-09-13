//! アプリ間で共有する自動化ルール・実行履歴の永続化契約。
//! 条件評価、プロンプト生成、発火判定、推論呼び出しは AgentOS が所有する。

use serde::{Deserialize, Serialize};

/// 自動化の結果 document につける `document_type`。
///
/// 利用者の記録（`memo`）と自動生成した結果を、すべてのアプリで区別する。
pub const DOCUMENT_TYPE_AUTOMATION_RESULT: &str = "automation_result";

/// lineage の actor に入る接頭辞。`automation:<rule_id>` の形で「どのルールが作ったか」を残す。
pub const ACTOR_PREFIX: &str = "automation:";

/// どこで推論を実行するか。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BackendKind {
    /// ローカルに置いた API キーで、提供元の HTTP API を直接呼ぶ。
    ApiKey,
    /// ブラウザ（WebView）上の AI にプロンプトを貼り付けて、応答を画面から読み取る。
    Browser,
}

impl BackendKind {
    pub fn as_str(self) -> &'static str {
        match self {
            BackendKind::ApiKey => "api_key",
            BackendKind::Browser => "browser",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "api_key" => Some(BackendKind::ApiKey),
            "browser" => Some(BackendKind::Browser),
            _ => None,
        }
    }
}

/// 何をきっかけに実行するか。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TriggerKind {
    /// 利用者が明示的に実行したときだけ動く。
    Manual,
    /// 条件に合う記録が現れたら動く。
    MetaMatch,
    /// cron の時刻で動く。対象の絞り込みには `metas` も併用できる。
    Schedule,
}

impl TriggerKind {
    pub fn as_str(self) -> &'static str {
        match self {
            TriggerKind::Manual => "manual",
            TriggerKind::MetaMatch => "meta_match",
            TriggerKind::Schedule => "schedule",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "manual" => Some(TriggerKind::Manual),
            "meta_match" => Some(TriggerKind::MetaMatch),
            "schedule" => Some(TriggerKind::Schedule),
            _ => None,
        }
    }
}

/// 実行のきっかけ。`trigger_config` の JSON をほどいた形。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Trigger {
    /// 対象を絞り込むメタ情報。空なら「すべての記録」。
    pub metas: Vec<MetaCondition>,
    /// `TriggerKind::Schedule` のときの cron 式。
    pub cron: Option<String>,
}

/// メタ情報1件ぶんの条件。
///
/// `value` が `None` ならラベルの一致だけを見る（`#タスク` は値の有無を問わず一致）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MetaCondition {
    pub label: String,
    pub value: Option<String>,
}

impl MetaCondition {
    pub fn label(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            value: None,
        }
    }
}

/// 自動化ルール1件。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutomationRule {
    pub id: String,
    pub workspace_id: String,
    pub name: String,
    pub description: Option<String>,
    pub prompt: String,
    pub backend: BackendKind,
    /// バックエンド固有の設定（`backend_config` の JSON をそのまま持つ）。
    pub backend_config: BackendConfig,
    pub trigger_kind: TriggerKind,
    pub trigger: Trigger,
    pub enabled: bool,
    pub created_at: String,
    pub updated_at: String,
}

/// バックエンドの設定。api_key と browser で必要な項目が違う。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct BackendConfig {
    /// 提供元の識別子（資格情報ストアの account 名にもなる）。例: `anthropic`。
    pub provider: String,
    /// api_key のときのモデル ID。未指定なら提供元ごとの既定を使う。
    pub model: Option<String>,
    /// api_key のときの思考の深さ（`low` / `medium` / `high` / `xhigh` / `max`）。
    pub effort: Option<String>,
}

/// 実行1回の状態。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    Running,
    Succeeded,
    Failed,
    /// モデル側が安全上の理由で応答を拒否した。失敗とは分けて記録する
    /// （プロンプトを直せば通る可能性があり、無限に再試行しても意味がないため）。
    Refused,
}

impl RunStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            RunStatus::Running => "running",
            RunStatus::Succeeded => "succeeded",
            RunStatus::Failed => "failed",
            RunStatus::Refused => "refused",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "running" => Some(RunStatus::Running),
            "succeeded" => Some(RunStatus::Succeeded),
            "failed" => Some(RunStatus::Failed),
            "refused" => Some(RunStatus::Refused),
            _ => None,
        }
    }
}

/// 実行1回の記録。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutomationRun {
    pub id: String,
    pub workspace_id: String,
    pub rule_id: String,
    pub source_document_id: String,
    pub result_document_id: Option<String>,
    pub status: RunStatus,
    pub backend: BackendKind,
    pub error: Option<String>,
    pub started_at: String,
    pub finished_at: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn backend_and_trigger_round_trip_through_their_stored_strings() {
        for backend in [BackendKind::ApiKey, BackendKind::Browser] {
            assert_eq!(BackendKind::parse(backend.as_str()), Some(backend));
        }
        for trigger in [
            TriggerKind::Manual,
            TriggerKind::MetaMatch,
            TriggerKind::Schedule,
        ] {
            assert_eq!(TriggerKind::parse(trigger.as_str()), Some(trigger));
        }
        for status in [
            RunStatus::Running,
            RunStatus::Succeeded,
            RunStatus::Failed,
            RunStatus::Refused,
        ] {
            assert_eq!(RunStatus::parse(status.as_str()), Some(status));
        }
        assert_eq!(BackendKind::parse("なにか"), None);
    }
}
