//! この機能のテストで使う道具立て。
//!
//! `run` と `backend` の両方がルールを組み立てるので、置き場をここ1つにする。

use anyhow::{Result, bail};

use crate::domain::automation::{
    AutomationRule, BackendConfig, BackendKind, InferenceOutcome, InferenceRequest, Trigger,
    TriggerKind,
};
use crate::domain::capture::DocumentAsset;
use crate::domain::meta::MetaAssignment;
use crate::domain::ports::{CaptureStore, CaptureTx, InferenceBackend};
use crate::infra::clock::{FixedClock, SequentialIds};
use crate::infra::crypto::Sha256Hasher;
use crate::infra::sqlite::Database;

use super::Automation;

/// 返す値を決め打ちにしたバックエンド。
pub struct StubBackend(pub Result<InferenceOutcome, &'static str>);

impl InferenceBackend for StubBackend {
    fn complete(&self, _request: &InferenceRequest) -> Result<InferenceOutcome> {
        match &self.0 {
            Ok(outcome) => Ok(outcome.clone()),
            Err(message) => bail!("{message}"),
        }
    }
}

pub struct Fixture {
    pub db: Database,
    pub clock: FixedClock,
    pub ids: SequentialIds,
    pub hasher: Sha256Hasher,
}

impl Fixture {
    pub fn new() -> Self {
        Self {
            db: Database::open_in_memory().unwrap(),
            clock: FixedClock::new("2026-08-13T09:00:00Z"),
            ids: SequentialIds::new(),
            hasher: Sha256Hasher,
        }
    }

    pub fn automation(&self) -> Automation<'_> {
        Automation {
            rules: &self.db,
            runs: &self.db,
            memos: &self.db,
            store: &self.db,
            clock: &self.clock,
            ids: &self.ids,
            hasher: &self.hasher,
        }
    }

    /// 記録を1件書く（minos が保存したものに相当）。
    pub fn write_memo(&self, id: &str, body: &str, metas: &[MetaAssignment]) {
        let document = DocumentAsset::memo(id, "ws", body, "2026-08-13T08:00:00Z");
        CaptureStore::transact(&self.db, &mut |tx: &mut dyn CaptureTx| {
            tx.ensure_workspace("ws", "minos", "2026-08-13T08:00:00Z")?;
            tx.insert_document(&document)?;
            for (index, meta) in metas.iter().enumerate() {
                tx.insert_document_meta(
                    &format!("{id}-meta-{index}"),
                    id,
                    meta,
                    "2026-08-13T08:00:00Z",
                )?;
            }
            Ok(())
        })
        .unwrap();
    }

    pub fn write_rule(&self, rule: &AutomationRule) {
        let conn = self.db.connection_for_test();
        conn.execute(
            "INSERT INTO automation_rules
                 (id, workspace_id, name, description, prompt, backend_kind, backend_config,
                  trigger_kind, trigger_config, enabled, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            rusqlite::params![
                rule.id,
                rule.workspace_id,
                rule.name,
                rule.description,
                rule.prompt,
                rule.backend.as_str(),
                serde_json::to_string(&rule.backend_config).unwrap(),
                rule.trigger_kind.as_str(),
                serde_json::to_string(&rule.trigger).unwrap(),
                rule.enabled as i64,
                rule.created_at,
                rule.updated_at,
            ],
        )
        .unwrap();
    }
}

pub fn rule(id: &str, trigger_kind: TriggerKind, trigger: Trigger) -> AutomationRule {
    AutomationRule {
        id: id.into(),
        workspace_id: "ws".into(),
        name: "要約".into(),
        description: None,
        prompt: "要約して: {{memo.body}}".into(),
        backend: BackendKind::ApiKey,
        backend_config: BackendConfig {
            provider: "anthropic".into(),
            model: None,
            effort: None,
        },
        trigger_kind,
        trigger,
        enabled: true,
        created_at: "2026-08-13T00:00:00Z".into(),
        updated_at: "2026-08-13T00:00:00Z".into(),
    }
}
