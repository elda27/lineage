//! 結果記録・lineage・実行状態の確定境界。実行・プロンプト生成は Runner が所有する。

use super::{LinkedDocument, save_document::validate_target};
use crate::domain::automation::{AutomationRun, RunStatus};
use crate::domain::lineage::LineageLedger;
use crate::domain::ports::{AutomationStore, AutomationTx};
use crate::domain::shared::{Hasher, IdGenerator};
use anyhow::{Result, ensure};

pub struct CommitAutomationResult<'a> {
    pub store: &'a dyn AutomationStore,
    pub ids: &'a dyn IdGenerator,
    pub hasher: &'a dyn Hasher,
}

impl CommitAutomationResult<'_> {
    pub fn execute(&self, run: &AutomationRun, result: Option<LinkedDocument>) -> Result<()> {
        ensure!(run.finished_at.is_some(), "完了時刻が必要です");
        match (&run.status, &result) {
            (RunStatus::Succeeded, Some(result)) => {
                validate_target(&result.document, &result.lineage)?;
                ensure!(
                    result.document.workspace_id == run.workspace_id
                        && run.result_document_id.as_deref() == Some(result.document.id.as_str())
                        && result.lineage.source_kind == "document"
                        && result.lineage.source_id == run.source_document_id,
                    "実行結果と記録・lineage が一致しません"
                );
            }
            (RunStatus::Failed | RunStatus::Refused, None) => {
                ensure!(
                    run.result_document_id.is_none(),
                    "失敗・拒否した実行に結果記録を指定できません"
                );
            }
            _ => anyhow::bail!("実行状態と結果記録が一致しません"),
        }
        self.store.transact(&mut |tx: &mut dyn AutomationTx| {
            if let Some(result) = &result {
                tx.insert_document(&result.document)?;
                let previous = tx.last_link(&run.workspace_id)?;
                let link = LineageLedger::new(self.hasher).append_next(
                    previous.as_ref(),
                    self.ids.new_id(),
                    result.lineage.clone(),
                );
                tx.append_link(&link)?;
            }
            tx.finish_run(run)
        })
    }
}
