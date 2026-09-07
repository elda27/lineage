//! 差分 mutation の検査と、Rust 所有の ID / 時刻を付与するユースケース。

use anyhow::{Result, ensure};

use crate::domain::automation::TriggerKind;
use crate::domain::mutation::{
    AutomationRuleInput, AutomationRulePatch, MutationOperation, MutationRequest, MutationResult,
    NullablePatch, TagPatch,
};
use crate::domain::ports::MutationStore;
use crate::domain::shared::Clock;

pub struct ApplyMutation<'a> {
    pub store: &'a dyn MutationStore,
    pub clock: &'a dyn Clock,
}

impl ApplyMutation<'_> {
    pub fn execute(&self, mut request: MutationRequest) -> Result<MutationResult> {
        ensure!(
            !request.operation_id.trim().is_empty(),
            "operationId は空にできません"
        );
        ensure!(
            !request.workspace_id.trim().is_empty(),
            "workspaceId は空にできません"
        );
        if let Some(revision) = request.base_revision {
            ensure!(revision >= 0, "baseRevision は0以上で指定してください");
        }

        let operation_id = request.operation_id.clone();
        if let MutationOperation::AutomationRuleCreate { rule_id, .. } = &mut request.operation {
            if rule_id.as_deref().is_none_or(str::is_empty) {
                // retry でも同じ entity ID になるよう、冪等キーから決定的に導出する。
                // transport が ruleId を発行する必要はなく、ID の決定権は Rust 側に残る。
                *rule_id = Some(operation_id);
            }
        }

        validate_operation(&request.operation)?;
        self.store
            .apply_mutation(&request, &self.clock.now_rfc3339())
    }
}

fn validate_operation(operation: &MutationOperation) -> Result<()> {
    match operation {
        MutationOperation::MemoStatePatch { memo_id, patch } => {
            non_empty_id("memoId", memo_id)?;
            ensure!(!patch.is_empty(), "memo state patch が空です");
        }
        MutationOperation::ArchiveCompletedTasks { labels } => {
            ensure!(!labels.is_empty(), "labels は1件以上必要です");
            ensure!(
                labels.iter().all(|label| !label.trim().is_empty()),
                "labels に空文字は指定できません"
            );
        }
        MutationOperation::TagPatch { tag_id, patch } => {
            non_empty_id("tagId", tag_id)?;
            validate_tag_patch(patch)?;
        }
        MutationOperation::TagDelete { tag_id } => non_empty_id("tagId", tag_id)?,
        MutationOperation::AutomationRuleCreate { rule_id, input } => {
            non_empty_id("ruleId", rule_id.as_deref().unwrap_or_default())?;
            validate_rule_input(input)?;
        }
        MutationOperation::AutomationRulePatch { rule_id, patch } => {
            non_empty_id("ruleId", rule_id)?;
            validate_rule_patch(patch)?;
        }
        MutationOperation::AutomationRuleDelete { rule_id } => non_empty_id("ruleId", rule_id)?,
        MutationOperation::SettingSet { key, .. } => non_empty_id("key", key)?,
    }
    Ok(())
}

fn validate_tag_patch(patch: &TagPatch) -> Result<()> {
    ensure!(!patch.is_empty(), "tag patch が空です");
    if let Some(display_name) = &patch.display_name {
        ensure!(
            !display_name.trim().is_empty(),
            "displayName は空にできません"
        );
    }
    if let NullablePatch::Set(view) = &patch.view {
        ensure!(
            !view.trim().is_empty(),
            "view は空文字ではなく null で解除してください"
        );
    }
    if let NullablePatch::Set(recipe) = &patch.recipe {
        ensure!(
            !recipe.name.trim().is_empty(),
            "recipe.name は空文字ではなく null で解除してください"
        );
    }
    Ok(())
}

fn validate_rule_input(input: &AutomationRuleInput) -> Result<()> {
    ensure!(!input.name.trim().is_empty(), "name は空にできません");
    ensure!(!input.prompt.trim().is_empty(), "prompt は空にできません");
    ensure!(
        !input.backend_config.provider.trim().is_empty(),
        "backendConfig.provider は空にできません"
    );
    if input.trigger_kind == TriggerKind::Schedule {
        ensure!(
            input
                .trigger
                .cron
                .as_deref()
                .is_some_and(|cron| !cron.trim().is_empty()),
            "schedule には trigger.cron が必要です"
        );
    }
    Ok(())
}

fn validate_rule_patch(patch: &AutomationRulePatch) -> Result<()> {
    ensure!(!patch.is_empty(), "automation rule patch が空です");
    if let Some(name) = &patch.name {
        ensure!(!name.trim().is_empty(), "name は空にできません");
    }
    if let Some(prompt) = &patch.prompt {
        ensure!(!prompt.trim().is_empty(), "prompt は空にできません");
    }
    if let Some(config) = &patch.backend_config {
        ensure!(
            !config.provider.trim().is_empty(),
            "backendConfig.provider は空にできません"
        );
    }
    if patch.trigger_kind == Some(TriggerKind::Schedule)
        && let Some(trigger) = &patch.trigger
    {
        ensure!(
            trigger
                .cron
                .as_deref()
                .is_some_and(|cron| !cron.trim().is_empty()),
            "schedule には trigger.cron が必要です"
        );
    }
    Ok(())
}

fn non_empty_id(name: &str, value: &str) -> Result<()> {
    ensure!(!value.trim().is_empty(), "{name} は空にできません");
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use super::*;
    use crate::domain::automation::{BackendConfig, BackendKind, Trigger};
    use crate::domain::mutation::{MutationStatus, TagRecipe};

    struct FixedClock;
    impl Clock for FixedClock {
        fn now_rfc3339(&self) -> String {
            "2026-08-25T00:00:00Z".into()
        }
    }

    #[derive(Default)]
    struct RecordingStore(RefCell<Vec<MutationRequest>>);
    impl MutationStore for RecordingStore {
        fn apply_mutation(
            &self,
            request: &MutationRequest,
            recorded_at: &str,
        ) -> Result<MutationResult> {
            self.0.borrow_mut().push(request.clone());
            Ok(MutationResult {
                operation_id: request.operation_id.clone(),
                status: MutationStatus::Applied,
                entity_kind: request.operation.entity_kind().into(),
                entity_id: request
                    .operation
                    .entity_id(&request.workspace_id)
                    .unwrap()
                    .into(),
                revision: 1,
                recorded_at: recorded_at.into(),
            })
        }
    }

    fn service(store: &RecordingStore) -> ApplyMutation<'_> {
        ApplyMutation {
            store,
            clock: &FixedClock,
        }
    }

    #[test]
    fn rust_generates_the_entity_id_for_rule_creation() {
        let store = RecordingStore::default();
        let request = MutationRequest {
            operation_id: "op-1".into(),
            workspace_id: "local".into(),
            base_revision: None,
            operation: MutationOperation::AutomationRuleCreate {
                rule_id: None,
                input: AutomationRuleInput {
                    name: "rule".into(),
                    description: None,
                    prompt: "prompt".into(),
                    backend: BackendKind::ApiKey,
                    backend_config: BackendConfig {
                        provider: "anthropic".into(),
                        ..Default::default()
                    },
                    trigger_kind: TriggerKind::Manual,
                    trigger: Trigger::default(),
                    enabled: true,
                },
            },
        };

        let result = service(&store).execute(request).unwrap();
        assert_eq!(result.entity_id, "op-1");
    }

    #[test]
    fn empty_patches_are_rejected_before_storage() {
        let store = RecordingStore::default();
        let request = MutationRequest {
            operation_id: "op-1".into(),
            workspace_id: "local".into(),
            base_revision: None,
            operation: MutationOperation::TagPatch {
                tag_id: "tag-1".into(),
                patch: TagPatch::default(),
            },
        };

        assert!(service(&store).execute(request).is_err());
        assert!(store.0.borrow().is_empty());
    }

    #[test]
    fn null_and_value_tag_changes_are_valid_deltas() {
        let store = RecordingStore::default();
        let request = MutationRequest {
            operation_id: "op-1".into(),
            workspace_id: "local".into(),
            base_revision: Some(3),
            operation: MutationOperation::TagPatch {
                tag_id: "tag-1".into(),
                patch: TagPatch {
                    shorthand: NullablePatch::Clear,
                    recipe: NullablePatch::Set(TagRecipe {
                        name: "build".into(),
                        managed: true,
                    }),
                    ..Default::default()
                },
            },
        };

        let result = service(&store).execute(request).unwrap();
        assert_eq!(result.revision, 1);
    }
}
