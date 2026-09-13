//! Runner が所有する条件評価・プロンプト展開・結果の命名。
use lineage_core::domain::automation::AutomationRule;
use lineage_core::domain::document::DocumentSnapshot;

/// `#ラベル` / `#ラベル=値` を並べた文字列。プロンプトへの埋め込みに使う。
fn meta_text(memo: &DocumentSnapshot) -> String {
    memo.metas
        .iter()
        .map(|meta| match &meta.value {
            Some(value) => format!("#{}={}", meta.label, value),
            None => format!("#{}", meta.label),
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// ルールが記録を対象にとるか。
///
/// 条件は AND で、すべて満たしたときだけ一致とする。条件が空なら「すべての記録」。
/// 無効なルールはここで落とす（呼び出し側で毎回 `enabled` を見なくて済むように）。
pub fn matches(rule: &AutomationRule, memo: &DocumentSnapshot) -> bool {
    if !rule.enabled {
        return false;
    }
    rule.trigger.metas.iter().all(|condition| {
        memo.metas.iter().any(|meta| {
            condition.label == meta.label
                && condition
                    .value
                    .as_ref()
                    .is_none_or(|value| meta.value.as_ref() == Some(value))
        })
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
pub fn render_prompt(rule: &AutomationRule, memo: &DocumentSnapshot, now: &str) -> String {
    rule.prompt
        .replace("{{memo.title}}", &memo.title)
        .replace("{{memo.body}}", &memo.body_text)
        .replace("{{memo.metas}}", &meta_text(memo))
        .replace("{{now}}", now)
}

/// 自動化の結果 document のタイトル。
///
/// 一覧で「どのルールが、どの記録から作ったか」が分かる必要があるので、両方入れる。
pub fn result_title(rule: &AutomationRule, memo: &DocumentSnapshot) -> String {
    format!("{}: {}", rule.name, memo.title)
}

#[cfg(test)]
mod tests {
    use super::*;
    use lineage_core::domain::automation::{
        BackendConfig, BackendKind, MetaCondition, Trigger, TriggerKind,
    };
    use lineage_core::domain::meta::MetaAssignment;

    fn memo(metas: Vec<MetaAssignment>) -> DocumentSnapshot {
        DocumentSnapshot {
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
        assert!(matches(
            &r,
            &memo(vec![MetaAssignment::user("タスク", None)])
        ));
        assert!(matches(
            &r,
            &memo(vec![MetaAssignment::user("タスク", Some("急ぎ".into()))])
        ));
        assert!(!matches(
            &r,
            &memo(vec![MetaAssignment::user("投資", None)])
        ));
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
        assert!(matches(
            &r,
            &memo(vec![MetaAssignment::auto("app", "chrome.exe")])
        ));
        assert!(!matches(
            &r,
            &memo(vec![MetaAssignment::auto("app", "code.exe")])
        ));
        // 値ありの条件は、値なしのメタ情報には一致しない。
        assert!(!matches(&r, &memo(vec![MetaAssignment::user("app", None)])));
    }

    #[test]
    fn conditions_are_combined_with_and() {
        let r = rule(Trigger {
            metas: vec![MetaCondition::label("タスク"), MetaCondition::label("投資")],
            cron: None,
        });
        assert!(!matches(
            &r,
            &memo(vec![MetaAssignment::user("タスク", None)])
        ));
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
}
