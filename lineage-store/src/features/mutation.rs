//! 差分 mutation の検査と、Rust 所有の ID / 時刻を付与するユースケース。

use anyhow::Result;

use crate::ports::Clock;
use crate::ports::MutationStore;
use lineage_core::domain::mutation::{MutationRequest, MutationResult};

pub struct ApplyMutation<'a> {
    pub store: &'a dyn MutationStore,
    pub clock: &'a dyn Clock,
}

impl ApplyMutation<'_> {
    pub fn execute(&self, mut request: MutationRequest) -> Result<MutationResult> {
        request.prepare()?;
        self.store
            .apply_mutation(&request, &self.clock.now_rfc3339())
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use super::*;
    use lineage_core::domain::automation::TriggerKind;
    use lineage_core::domain::automation::{BackendConfig, BackendKind, Trigger};
    use lineage_core::domain::mutation::{
        AutomationRuleInput, MutationOperation, NullablePatch, TagPatch,
    };
    use lineage_core::domain::mutation::{MutationStatus, TagRecipe};

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
