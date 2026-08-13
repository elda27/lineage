//! 自動化の実行。
//!
//! 「ルール＋記録 → プロンプト → 生成AI → 結果 document ＋ lineage」という流れを組み立てる。
//! バックエンドの違い（API キーで HTTP を叩くか、ブラウザに貼り付けるか）は
//! `InferenceBackend` の裏に隠れているので、ここには現れない。
//!
//! ブラウザ方式だけは実行部が fullos（WebView を持つ側）に出るため、
//! `begin` → 外で実行 → `finish_*` という分割した入口も用意してある。

use anyhow::{Context, Result};

use crate::domain::automation::{
    ACTOR_PREFIX, AutomationRule, AutomationRun, DOCUMENT_TYPE_AUTOMATION_RESULT, InferenceOutcome,
    InferenceRequest, MemoSnapshot, RunStatus, TriggerKind, matches, render_prompt, result_title,
};
use crate::domain::capture::DocumentAsset;
use crate::domain::lineage::{LineageInput, LineageLedger, relation};
use crate::domain::ports::{
    AutomationRuleQuery, AutomationRunStore, AutomationStore, AutomationTx, InferenceBackend,
    MemoQuery,
};
use crate::domain::shared::{Clock, Hasher, IdGenerator};

use super::schedule::{is_due, parse_time};

/// lineage の source/target に使う種別。
const KIND_DOCUMENT: &str = "document";

/// `pending` が1ルールあたり走査する記録の上限。
///
/// 条件の判定は domain の `matches` に寄せてあり、SQL では「未処理か」しか見ない。
/// そのぶん候補を多めに取る必要があるので、ここで上限を切って青天井にしない。
const PENDING_SCAN_LIMIT: usize = 200;

/// 自動化のユースケース群。
pub struct Automation<'a> {
    pub rules: &'a dyn AutomationRuleQuery,
    pub runs: &'a dyn AutomationRunStore,
    pub memos: &'a dyn MemoQuery,
    pub store: &'a dyn AutomationStore,
    pub clock: &'a dyn Clock,
    pub ids: &'a dyn IdGenerator,
    pub hasher: &'a dyn Hasher,
}

impl Automation<'_> {
    /// 記録1件に対して実行できるルール。メモの隣の「Action」ボタンが使う。
    ///
    /// 無効なルールと条件に合わないルールは `matches` が落とす。
    /// スケジュール実行のルールも、条件さえ合えば手動で流せるように含める。
    pub fn matching_rules(
        &self,
        workspace_id: &str,
        memo_id: &str,
    ) -> Result<Vec<AutomationRule>> {
        let memo = self.require_memo(workspace_id, memo_id)?;
        Ok(self
            .rules
            .all(workspace_id)?
            .into_iter()
            .filter(|rule| matches(rule, &memo))
            .collect())
    }

    /// ルールと記録から、実際に送るプロンプトを組み立てる。
    ///
    /// ブラウザ方式では fullos がこれを受け取って WebView に貼り付ける。
    pub fn prompt(&self, workspace_id: &str, rule_id: &str, memo_id: &str) -> Result<String> {
        let rule = self.require_rule(rule_id)?;
        let memo = self.require_memo(workspace_id, memo_id)?;
        Ok(render_prompt(&rule, &memo, &self.clock.now_rfc3339()))
    }

    /// API キー方式で1件実行し、結果を確定する。
    pub fn run(
        &self,
        workspace_id: &str,
        rule_id: &str,
        memo_id: &str,
        backend: &dyn InferenceBackend,
    ) -> Result<AutomationRun> {
        let rule = self.require_rule(rule_id)?;
        let memo = self.require_memo(workspace_id, memo_id)?;
        self.run_rule(&rule, &memo, backend)
    }

    /// ルールと記録が手元にあるときの実行本体（`poll` から使う）。
    pub fn run_rule(
        &self,
        rule: &AutomationRule,
        memo: &MemoSnapshot,
        backend: &dyn InferenceBackend,
    ) -> Result<AutomationRun> {
        let run = self.begin(rule, memo)?;
        let prompt = render_prompt(rule, memo, &run.started_at);
        let request = InferenceRequest {
            provider: rule.backend_config.provider.clone(),
            prompt,
            model: rule.backend_config.model.clone(),
            effort: rule.backend_config.effort.clone(),
        };

        // 実行が失敗しても run を running のまま放置しない。失敗として確定させないと、
        // 「実行中」の行が残り続けて以後その記録が二度と拾われなくなる。
        match backend.complete(&request) {
            Ok(InferenceOutcome::Completed(text)) => self.finish_success(run, rule, memo, &text),
            Ok(InferenceOutcome::Refused { category }) => self.finish_refused(run, category),
            Err(error) => self.finish_failure(run, &format!("{error:#}")),
        }
    }

    /// 外部（ブラウザ WebView）で得た結果を確定する。
    pub fn record(
        &self,
        workspace_id: &str,
        rule_id: &str,
        memo_id: &str,
        text: &str,
    ) -> Result<AutomationRun> {
        let rule = self.require_rule(rule_id)?;
        let memo = self.require_memo(workspace_id, memo_id)?;
        let run = self.begin(&rule, &memo)?;
        self.finish_success(run, &rule, &memo, text)
    }

    /// 外部での実行が失敗したことを記録する（ブラウザを閉じられた場合など）。
    pub fn record_failure(
        &self,
        workspace_id: &str,
        rule_id: &str,
        memo_id: &str,
        error: &str,
    ) -> Result<AutomationRun> {
        let rule = self.require_rule(rule_id)?;
        let memo = self.require_memo(workspace_id, memo_id)?;
        let run = self.begin(&rule, &memo)?;
        self.finish_failure(run, error)
    }

    /// メタ情報マッチのルールについて、まだ処理していない記録を返す。
    pub fn pending(&self, workspace_id: &str, rule: &AutomationRule) -> Result<Vec<MemoSnapshot>> {
        if !rule.enabled {
            return Ok(Vec::new());
        }
        Ok(self
            .runs
            .unprocessed_memos(workspace_id, &rule.id, PENDING_SCAN_LIMIT)?
            .into_iter()
            .filter(|memo| matches(rule, memo))
            .collect())
    }

    /// メタ情報マッチのルールのうち、有効なもの。
    pub fn meta_match_rules(&self, workspace_id: &str) -> Result<Vec<AutomationRule>> {
        Ok(self
            .rules
            .all(workspace_id)?
            .into_iter()
            .filter(|rule| rule.enabled && rule.trigger_kind == TriggerKind::MetaMatch)
            .collect())
    }

    /// スケジュール実行のルールのうち、いま発火すべきもの。
    ///
    /// 「前回の実行開始より後に発火時刻があるか」で判定する。別に状態ファイルを持たず
    /// `automation_runs` だけで決めるので、agentos は状態を持たないままでいられる。
    pub fn due_schedules(&self, workspace_id: &str) -> Result<Vec<AutomationRule>> {
        let now = parse_time(&self.clock.now_rfc3339())?;
        let mut due = Vec::new();
        for rule in self.rules.all(workspace_id)? {
            if !rule.enabled || rule.trigger_kind != TriggerKind::Schedule {
                continue;
            }
            let Some(expression) = rule.trigger.cron.as_deref() else {
                continue;
            };
            let last = match self.runs.last_started_at(&rule.id)? {
                Some(value) => parse_time(&value)?,
                // 一度も実行していないルールは、次の発火時刻まで待つ。過去にさかのぼって
                // まとめ実行すると、ルールを作った瞬間に大量の実行が走ってしまう。
                None => now,
            };
            if is_due(expression, last, now)? {
                due.push(rule);
            }
        }
        Ok(due)
    }

    /// 実行を `running` として記録する。
    fn begin(&self, rule: &AutomationRule, memo: &MemoSnapshot) -> Result<AutomationRun> {
        let run = AutomationRun {
            id: self.ids.new_id(),
            workspace_id: rule.workspace_id.clone(),
            rule_id: rule.id.clone(),
            source_document_id: memo.id.clone(),
            result_document_id: None,
            status: RunStatus::Running,
            backend: rule.backend,
            error: None,
            started_at: self.clock.now_rfc3339(),
            finished_at: None,
        };
        self.runs.start(&run)?;
        Ok(run)
    }

    /// 結果 document・lineage の link・run の確定を同一トランザクションで行う。
    ///
    /// 分けると、結果だけ保存されて鎖に載らない（あるいはその逆の）状態が起こりうる。
    fn finish_success(
        &self,
        mut run: AutomationRun,
        rule: &AutomationRule,
        memo: &MemoSnapshot,
        text: &str,
    ) -> Result<AutomationRun> {
        let now = self.clock.now_rfc3339();
        let result = DocumentAsset {
            id: self.ids.new_id(),
            workspace_id: rule.workspace_id.clone(),
            title: result_title(rule, memo),
            body_text: text.to_string(),
            document_type: DOCUMENT_TYPE_AUTOMATION_RESULT.to_string(),
            created_at: now.clone(),
            updated_at: now.clone(),
        };

        let lineage_input = LineageInput {
            workspace_id: rule.workspace_id.clone(),
            source_kind: KIND_DOCUMENT.to_string(),
            source_id: memo.id.clone(),
            target_kind: KIND_DOCUMENT.to_string(),
            target_id: result.id.clone(),
            relation_type: relation::DERIVED_FROM.to_string(),
            actor: format!("{ACTOR_PREFIX}{}", rule.id),
            created_at: now.clone(),
        };

        run.status = RunStatus::Succeeded;
        run.result_document_id = Some(result.id.clone());
        run.finished_at = Some(now);

        let finished = run.clone();
        self.store.transact(&mut |tx: &mut dyn AutomationTx| {
            tx.insert_document(&result)?;
            let prev = tx.last_link(&rule.workspace_id)?;
            let link = LineageLedger::new(self.hasher).append_next(
                prev.as_ref(),
                self.ids.new_id(),
                lineage_input.clone(),
            );
            tx.append_link(&link)?;
            tx.finish_run(&finished)?;
            Ok(())
        })?;

        Ok(run)
    }

    fn finish_failure(&self, mut run: AutomationRun, error: &str) -> Result<AutomationRun> {
        run.status = RunStatus::Failed;
        run.error = Some(error.to_string());
        run.finished_at = Some(self.clock.now_rfc3339());
        self.finish_without_result(run)
    }

    fn finish_refused(
        &self,
        mut run: AutomationRun,
        category: Option<String>,
    ) -> Result<AutomationRun> {
        run.status = RunStatus::Refused;
        run.error = Some(match category {
            Some(category) => format!("応答が拒否されました（{category}）"),
            None => "応答が拒否されました".to_string(),
        });
        run.finished_at = Some(self.clock.now_rfc3339());
        self.finish_without_result(run)
    }

    fn finish_without_result(&self, run: AutomationRun) -> Result<AutomationRun> {
        let finished = run.clone();
        self.store.transact(&mut |tx: &mut dyn AutomationTx| {
            tx.finish_run(&finished)?;
            Ok(())
        })?;
        Ok(run)
    }

    fn require_rule(&self, rule_id: &str) -> Result<AutomationRule> {
        self.rules
            .get(rule_id)?
            .with_context(|| format!("自動化ルールが見つかりません: {rule_id}"))
    }

    fn require_memo(&self, workspace_id: &str, memo_id: &str) -> Result<MemoSnapshot> {
        self.memos
            .get(workspace_id, memo_id)?
            .with_context(|| format!("記録が見つかりません: {memo_id}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::automation::test_support::{Fixture, StubBackend, rule};
    use crate::domain::automation::{MetaCondition, Trigger};
    use crate::domain::lineage::VerifyResult;
    use crate::domain::meta::MetaAssignment;
    use crate::domain::ports::LineageQuery;

    #[test]
    fn a_successful_run_stores_the_result_and_extends_the_chain() {
        let f = Fixture::new();
        f.write_memo("doc-1", "SOXL 損切り\n理由は決算", &[]);
        f.write_rule(&rule("rule-1", TriggerKind::Manual, Trigger::default()));

        let backend = StubBackend(Ok(InferenceOutcome::Completed("3行の要約".into())));
        let run = f
            .automation()
            .run("ws", "rule-1", "doc-1", &backend)
            .unwrap();

        assert_eq!(run.status, RunStatus::Succeeded);
        let result_id = run.result_document_id.clone().unwrap();

        // 結果は memo とは別の document として保存される。
        let conn = f.db.connection_for_test();
        let (title, body, kind): (String, String, String) = conn
            .query_row(
                "SELECT title, body_text, document_type FROM documents WHERE id = ?1",
                rusqlite::params![result_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(title, "要約: SOXL 損切り");
        assert_eq!(body, "3行の要約");
        assert_eq!(kind, DOCUMENT_TYPE_AUTOMATION_RESULT);
        drop(conn);

        // memo → 結果 の link が derived_from で1本だけ増え、鎖は繋がっている。
        let records = LineageQuery::list(&f.db, "ws").unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].source_id, "doc-1");
        assert_eq!(records[0].target_id, result_id);
        assert_eq!(records[0].relation_type, relation::DERIVED_FROM);
        assert_eq!(records[0].actor, "automation:rule-1");
        assert_eq!(
            LineageLedger::new(&f.hasher).verify(&records),
            VerifyResult::Ok { checked: 1 }
        );
    }

    #[test]
    fn a_backend_error_is_recorded_as_a_failed_run() {
        let f = Fixture::new();
        f.write_memo("doc-1", "メモ", &[]);
        f.write_rule(&rule("rule-1", TriggerKind::Manual, Trigger::default()));

        let backend = StubBackend(Err("鍵が未登録です"));
        let run = f
            .automation()
            .run("ws", "rule-1", "doc-1", &backend)
            .unwrap();

        assert_eq!(run.status, RunStatus::Failed);
        assert!(run.error.unwrap().contains("鍵が未登録"));
        assert!(run.result_document_id.is_none());
        // 失敗した実行では鎖に何も足さない。
        assert!(LineageQuery::list(&f.db, "ws").unwrap().is_empty());
    }

    #[test]
    fn a_refusal_is_recorded_separately_from_a_failure() {
        let f = Fixture::new();
        f.write_memo("doc-1", "メモ", &[]);
        f.write_rule(&rule("rule-1", TriggerKind::Manual, Trigger::default()));

        let backend = StubBackend(Ok(InferenceOutcome::Refused {
            category: Some("cyber".into()),
        }));
        let run = f
            .automation()
            .run("ws", "rule-1", "doc-1", &backend)
            .unwrap();

        assert_eq!(run.status, RunStatus::Refused);
        assert!(run.error.unwrap().contains("cyber"));
    }

    #[test]
    fn pending_skips_memos_that_already_succeeded() {
        let f = Fixture::new();
        f.write_memo("doc-1", "メモ1", &[MetaAssignment::user("タスク", None)]);
        f.write_memo("doc-2", "メモ2", &[MetaAssignment::user("タスク", None)]);
        // 条件に合わない記録は最初から対象外。
        f.write_memo("doc-3", "メモ3", &[MetaAssignment::user("投資", None)]);

        let r = rule(
            "rule-1",
            TriggerKind::MetaMatch,
            Trigger {
                metas: vec![MetaCondition::label("タスク")],
                cron: None,
            },
        );
        f.write_rule(&r);
        let automation = f.automation();

        let pending = automation.pending("ws", &r).unwrap();
        assert_eq!(pending.len(), 2);

        let backend = StubBackend(Ok(InferenceOutcome::Completed("要約".into())));
        automation.run("ws", "rule-1", "doc-1", &backend).unwrap();

        // 成功した記録はもう返らない（＝二重実行しない）。
        let pending = automation.pending("ws", &r).unwrap();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].id, "doc-2");
    }

    #[test]
    fn pending_returns_a_failed_memo_again_so_it_can_be_retried() {
        let f = Fixture::new();
        f.write_memo("doc-1", "メモ", &[]);
        let r = rule("rule-1", TriggerKind::MetaMatch, Trigger::default());
        f.write_rule(&r);
        let automation = f.automation();

        let backend = StubBackend(Err("通信できません"));
        automation.run("ws", "rule-1", "doc-1", &backend).unwrap();

        assert_eq!(automation.pending("ws", &r).unwrap().len(), 1);
    }

    #[test]
    fn matching_rules_filters_by_the_memos_metadata() {
        let f = Fixture::new();
        f.write_memo("doc-1", "メモ", &[MetaAssignment::user("タスク", None)]);
        f.write_rule(&rule(
            "rule-task",
            TriggerKind::Manual,
            Trigger {
                metas: vec![MetaCondition::label("タスク")],
                cron: None,
            },
        ));
        f.write_rule(&rule(
            "rule-idea",
            TriggerKind::Manual,
            Trigger {
                metas: vec![MetaCondition::label("アイデア")],
                cron: None,
            },
        ));

        let found = f.automation().matching_rules("ws", "doc-1").unwrap();
        assert_eq!(
            found.iter().map(|r| r.id.as_str()).collect::<Vec<_>>(),
            vec!["rule-task"]
        );
    }

    #[test]
    fn the_prompt_gets_the_memos_body_and_metadata() {
        let f = Fixture::new();
        f.write_memo("doc-1", "本文", &[MetaAssignment::user("タスク", None)]);
        let mut r = rule("rule-1", TriggerKind::Manual, Trigger::default());
        r.prompt = "{{memo.metas}} / {{memo.body}}".into();
        f.write_rule(&r);

        assert_eq!(
            f.automation().prompt("ws", "rule-1", "doc-1").unwrap(),
            "#タスク / 本文"
        );
    }

    #[test]
    fn an_unknown_rule_or_memo_is_reported_rather_than_silently_skipped() {
        let f = Fixture::new();
        let automation = f.automation();
        assert!(automation.prompt("ws", "missing", "doc-1").is_err());

        f.write_rule(&rule("rule-1", TriggerKind::Manual, Trigger::default()));
        assert!(automation.prompt("ws", "rule-1", "missing").is_err());
    }

    #[test]
    fn a_rule_that_never_ran_waits_for_its_next_occurrence() {
        let f = Fixture::new();
        let mut r = rule(
            "rule-1",
            TriggerKind::Schedule,
            Trigger {
                metas: Vec::new(),
                cron: Some("0 0 * * * *".into()),
            },
        );
        r.prompt = "定型".into();
        f.write_rule(&r);

        // 一度も実行していないルールは、作った直後には発火しない
        // （過去にさかのぼってまとめ実行しない）。
        assert!(f.automation().due_schedules("ws").unwrap().is_empty());
    }
}
