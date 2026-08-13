//! 自動化ルールと、その「対象を選ぶ」「プロンプトを組み立てる」規則。
//!
//! docs/ui.md「自動化画面」に対応する。ここは domain なので DB / HTTP / OS には触れない。
//! 実際にモデルを呼ぶのは infrastructure、順序を決めるのは application。
//!
//! 自動化の結果は memo とは別の document として保存し、memo → 結果 の link を
//! `derived_from` で張る。つまり自動生成物も「何から作られたか」を辿れる。

use serde::{Deserialize, Serialize};

use crate::domain::meta::MetaAssignment;

/// 自動化の結果 document につける `document_type`。
///
/// minos が書く記録（`memo`）と混ざると一覧に自動生成物が並んでしまうので、別の型にする。
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

    /// この条件が、記録に付いたメタ情報1件に一致するか。
    fn matches_assignment(&self, meta: &MetaAssignment) -> bool {
        if self.label != meta.label {
            return false;
        }
        match &self.value {
            None => true,
            Some(expected) => meta.value.as_deref() == Some(expected.as_str()),
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

/// プロンプトを組み立てるために必要な、記録の側の情報。
///
/// `DocumentAsset` をそのまま受けないのは、メタ情報が別テーブルにあり、
/// 自動化からは「本文＋メタ情報」が1つの塊に見えていてほしいため。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoSnapshot {
    pub id: String,
    pub title: String,
    pub body_text: String,
    pub metas: Vec<MetaAssignment>,
    pub created_at: String,
}

impl MemoSnapshot {
    /// `#ラベル` / `#ラベル=値` を並べた文字列。プロンプトへの埋め込みに使う。
    pub fn meta_text(&self) -> String {
        self.metas
            .iter()
            .map(|meta| match &meta.value {
                Some(value) => format!("#{}={}", meta.label, value),
                None => format!("#{}", meta.label),
            })
            .collect::<Vec<_>>()
            .join(" ")
    }
}

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

/// ルールが記録を対象にとるか。
///
/// 条件は AND で、すべて満たしたときだけ一致とする。条件が空なら「すべての記録」。
/// 無効なルールはここで落とす（呼び出し側で毎回 `enabled` を見なくて済むように）。
pub fn matches(rule: &AutomationRule, memo: &MemoSnapshot) -> bool {
    if !rule.enabled {
        return false;
    }
    rule.trigger.metas.iter().all(|condition| {
        memo.metas
            .iter()
            .any(|meta| condition.matches_assignment(meta))
    })
}

/// プロンプトのテンプレートに記録を差し込む。
///
/// 使えるプレースホルダは以下の4つだけにしておく。式や条件分岐まで持ち込むと
/// テンプレート言語の実装と保守が必要になり、自動化の本題から外れるため。
///
/// - `{{memo.title}}` … 記録のタイトル（本文1行目）
/// - `{{memo.body}}`  … 本文そのまま
/// - `{{memo.metas}}` … `#タスク #app=chrome.exe` のような文字列
/// - `{{now}}`        … 実行時刻（RFC3339）
///
/// プレースホルダを1つも含まないテンプレートは、そのまま定型の指示として使える。
pub fn render_prompt(rule: &AutomationRule, memo: &MemoSnapshot, now: &str) -> String {
    rule.prompt
        .replace("{{memo.title}}", &memo.title)
        .replace("{{memo.body}}", &memo.body_text)
        .replace("{{memo.metas}}", &memo.meta_text())
        .replace("{{now}}", now)
}

/// 自動化の結果 document のタイトル。
///
/// 一覧で「どのルールが、どの記録から作ったか」が分かる必要があるので、両方入れる。
pub fn result_title(rule: &AutomationRule, memo: &MemoSnapshot) -> String {
    format!("{}: {}", rule.name, memo.title)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn memo(metas: Vec<MetaAssignment>) -> MemoSnapshot {
        MemoSnapshot {
            id: "doc-1".into(),
            title: "SOXL 損切り".into(),
            body_text: "SOXL 損切り\n理由は決算前のボラ".into(),
            metas,
            created_at: "2026-08-13T00:00:00Z".into(),
        }
    }

    fn rule(trigger: Trigger) -> AutomationRule {
        AutomationRule {
            id: "rule-1".into(),
            workspace_id: "ws".into(),
            name: "要約".into(),
            description: None,
            prompt: "次の記録を要約して:\n{{memo.body}}".into(),
            backend: BackendKind::ApiKey,
            backend_config: BackendConfig {
                provider: "anthropic".into(),
                model: None,
                effort: None,
            },
            trigger_kind: TriggerKind::MetaMatch,
            trigger,
            enabled: true,
            created_at: "2026-08-13T00:00:00Z".into(),
            updated_at: "2026-08-13T00:00:00Z".into(),
        }
    }

    #[test]
    fn a_rule_without_conditions_takes_every_memo() {
        assert!(matches(&rule(Trigger::default()), &memo(Vec::new())));
    }

    #[test]
    fn a_label_condition_ignores_the_value() {
        let r = rule(Trigger {
            metas: vec![MetaCondition::label("タスク")],
            cron: None,
        });
        assert!(matches(&r, &memo(vec![MetaAssignment::user("タスク", None)])));
        assert!(matches(
            &r,
            &memo(vec![MetaAssignment::user("タスク", Some("急ぎ".into()))])
        ));
        assert!(!matches(&r, &memo(vec![MetaAssignment::user("投資", None)])));
    }

    #[test]
    fn a_value_condition_needs_an_exact_value() {
        let r = rule(Trigger {
            metas: vec![MetaCondition {
                label: "app".into(),
                value: Some("chrome.exe".into()),
            }],
            cron: None,
        });
        assert!(matches(&r, &memo(vec![MetaAssignment::auto("app", "chrome.exe")])));
        assert!(!matches(&r, &memo(vec![MetaAssignment::auto("app", "code.exe")])));
        // 値ありの条件は、値なしのメタ情報には一致しない。
        assert!(!matches(&r, &memo(vec![MetaAssignment::user("app", None)])));
    }

    #[test]
    fn conditions_are_combined_with_and() {
        let r = rule(Trigger {
            metas: vec![MetaCondition::label("タスク"), MetaCondition::label("投資")],
            cron: None,
        });
        assert!(!matches(&r, &memo(vec![MetaAssignment::user("タスク", None)])));
        assert!(matches(
            &r,
            &memo(vec![
                MetaAssignment::user("タスク", None),
                MetaAssignment::user("投資", None),
            ])
        ));
    }

    #[test]
    fn a_disabled_rule_never_matches() {
        let mut r = rule(Trigger::default());
        r.enabled = false;
        assert!(!matches(&r, &memo(Vec::new())));
    }

    #[test]
    fn renders_every_placeholder() {
        let mut r = rule(Trigger::default());
        r.prompt = "[{{memo.title}}] {{memo.metas}} @{{now}}\n{{memo.body}}".into();
        let m = memo(vec![
            MetaAssignment::user("タスク", None),
            MetaAssignment::auto("app", "chrome.exe"),
        ]);

        assert_eq!(
            render_prompt(&r, &m, "2026-08-13T09:00:00Z"),
            "[SOXL 損切り] #タスク #app=chrome.exe @2026-08-13T09:00:00Z\nSOXL 損切り\n理由は決算前のボラ"
        );
    }

    #[test]
    fn a_template_without_placeholders_is_used_as_is() {
        let mut r = rule(Trigger::default());
        r.prompt = "今日のタスクを3行でまとめて".into();
        assert_eq!(
            render_prompt(&r, &memo(Vec::new()), "2026-08-13T09:00:00Z"),
            "今日のタスクを3行でまとめて"
        );
    }

    #[test]
    fn the_result_title_names_both_the_rule_and_the_memo() {
        assert_eq!(
            result_title(&rule(Trigger::default()), &memo(Vec::new())),
            "要約: SOXL 損切り"
        );
    }

    #[test]
    fn backend_and_trigger_round_trip_through_their_stored_strings() {
        for backend in [BackendKind::ApiKey, BackendKind::Browser] {
            assert_eq!(BackendKind::parse(backend.as_str()), Some(backend));
        }
        for trigger in [TriggerKind::Manual, TriggerKind::MetaMatch, TriggerKind::Schedule] {
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
