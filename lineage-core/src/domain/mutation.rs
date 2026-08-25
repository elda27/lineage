//! FullOS からローカル永続化へ渡す差分 mutation の契約。
//!
//! WebView に SQL を公開せず、変更されたフィールドだけを Rust 側へ渡す。`operation_id`
//! は再送の冪等性、`base_revision` は将来の local/server 同期時の競合検出に使う。

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::domain::automation::{BackendConfig, BackendKind, Trigger, TriggerKind};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MutationRequest {
    pub operation_id: String,
    pub workspace_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_revision: Option<i64>,
    pub operation: MutationOperation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum MutationOperation {
    MemoStatePatch {
        memo_id: String,
        patch: MemoStatePatch,
    },
    ArchiveCompletedTasks {
        labels: Vec<String>,
    },
    TagPatch {
        tag_id: String,
        patch: TagPatch,
    },
    TagDelete {
        tag_id: String,
    },
    AutomationRuleCreate {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        rule_id: Option<String>,
        input: AutomationRuleInput,
    },
    AutomationRulePatch {
        rule_id: String,
        patch: AutomationRulePatch,
    },
    AutomationRuleDelete {
        rule_id: String,
    },
    SettingSet {
        key: String,
        value: String,
    },
}

impl MutationOperation {
    pub fn operation_kind(&self) -> &'static str {
        match self {
            Self::MemoStatePatch { .. } => "memo_state_patch",
            Self::ArchiveCompletedTasks { .. } => "archive_completed_tasks",
            Self::TagPatch { .. } => "tag_patch",
            Self::TagDelete { .. } => "tag_delete",
            Self::AutomationRuleCreate { .. } => "automation_rule_create",
            Self::AutomationRulePatch { .. } => "automation_rule_patch",
            Self::AutomationRuleDelete { .. } => "automation_rule_delete",
            Self::SettingSet { .. } => "setting_set",
        }
    }

    pub fn entity_kind(&self) -> &'static str {
        match self {
            Self::MemoStatePatch { .. } => "memo_state",
            Self::ArchiveCompletedTasks { .. } => "workspace",
            Self::TagPatch { .. } | Self::TagDelete { .. } => "tag",
            Self::AutomationRuleCreate { .. }
            | Self::AutomationRulePatch { .. }
            | Self::AutomationRuleDelete { .. } => "automation_rule",
            Self::SettingSet { .. } => "setting",
        }
    }

    pub fn entity_id<'a>(&'a self, workspace_id: &'a str) -> Option<&'a str> {
        match self {
            Self::MemoStatePatch { memo_id, .. } => Some(memo_id),
            Self::ArchiveCompletedTasks { .. } => Some(workspace_id),
            Self::TagPatch { tag_id, .. } | Self::TagDelete { tag_id } => Some(tag_id),
            Self::AutomationRuleCreate { rule_id, .. } => rule_id.as_deref(),
            Self::AutomationRulePatch { rule_id, .. }
            | Self::AutomationRuleDelete { rule_id } => Some(rule_id),
            Self::SettingSet { key, .. } => Some(key),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MemoStatePatch {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub done: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub archived: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trashed: Option<bool>,
}

impl MemoStatePatch {
    pub fn is_empty(&self) -> bool {
        self.done.is_none() && self.archived.is_none() && self.trashed.is_none()
    }
}

/// `Unchanged`（JSON キー無し）、`Clear`（JSON null）、`Set(value)` を区別する。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum NullablePatch<T> {
    #[default]
    Unchanged,
    Clear,
    Set(T),
}

impl<T> NullablePatch<T> {
    pub fn is_unchanged(&self) -> bool {
        matches!(self, Self::Unchanged)
    }
}

impl<T: Serialize> Serialize for NullablePatch<T> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            Self::Unchanged | Self::Clear => serializer.serialize_none(),
            Self::Set(value) => value.serialize(serializer),
        }
    }
}

impl<'de, T: Deserialize<'de>> Deserialize<'de> for NullablePatch<T> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(match Option::<T>::deserialize(deserializer)? {
            Some(value) => Self::Set(value),
            None => Self::Clear,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TagPatch {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(default, skip_serializing_if = "NullablePatch::is_unchanged")]
    pub shorthand: NullablePatch<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    #[serde(default, skip_serializing_if = "NullablePatch::is_unchanged")]
    pub view: NullablePatch<String>,
    #[serde(default, skip_serializing_if = "NullablePatch::is_unchanged")]
    pub recipe: NullablePatch<TagRecipe>,
}

impl TagPatch {
    pub fn is_empty(&self) -> bool {
        self.display_name.is_none()
            && self.shorthand.is_unchanged()
            && self.enabled.is_none()
            && self.view.is_unchanged()
            && self.recipe.is_unchanged()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TagRecipe {
    pub name: String,
    pub managed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AutomationRuleInput {
    pub name: String,
    pub description: Option<String>,
    pub prompt: String,
    pub backend: BackendKind,
    pub backend_config: BackendConfig,
    pub trigger_kind: TriggerKind,
    pub trigger: Trigger,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AutomationRulePatch {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "NullablePatch::is_unchanged")]
    pub description: NullablePatch<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub backend: Option<BackendKind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub backend_config: Option<BackendConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trigger_kind: Option<TriggerKind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trigger: Option<Trigger>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
}

impl AutomationRulePatch {
    pub fn is_empty(&self) -> bool {
        self.name.is_none()
            && self.description.is_unchanged()
            && self.prompt.is_none()
            && self.backend.is_none()
            && self.backend_config.is_none()
            && self.trigger_kind.is_none()
            && self.trigger.is_none()
            && self.enabled.is_none()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MutationStatus {
    Applied,
    Duplicate,
    Conflict,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MutationResult {
    pub operation_id: String,
    pub status: MutationStatus,
    pub entity_kind: String,
    pub entity_id: String,
    pub revision: i64,
    pub recorded_at: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nullable_patch_distinguishes_missing_null_and_value() {
        let missing: TagPatch = serde_json::from_str(r#"{}"#).unwrap();
        assert_eq!(missing.shorthand, NullablePatch::Unchanged);

        let clear: TagPatch = serde_json::from_str(r#"{"shorthand":null}"#).unwrap();
        assert_eq!(clear.shorthand, NullablePatch::Clear);

        let set: TagPatch = serde_json::from_str(r#"{"shorthand":"t"}"#).unwrap();
        assert_eq!(set.shorthand, NullablePatch::Set("t".into()));
    }

    #[test]
    fn patch_serialization_omits_unchanged_but_keeps_clear() {
        let patch = TagPatch {
            shorthand: NullablePatch::Clear,
            ..Default::default()
        };
        assert_eq!(serde_json::to_value(patch).unwrap(), serde_json::json!({"shorthand": null}));
    }

    #[test]
    fn request_uses_the_typescript_camel_case_contract() {
        let request: MutationRequest = serde_json::from_value(serde_json::json!({
            "operationId": "op-1",
            "workspaceId": "local",
            "operation": {
                "type": "memo_state_patch",
                "memoId": "memo-1",
                "patch": { "done": true }
            }
        }))
        .unwrap();

        assert!(matches!(
            request.operation,
            MutationOperation::MemoStatePatch { memo_id, .. } if memo_id == "memo-1"
        ));
    }

    #[test]
    fn result_uses_the_typescript_camel_case_contract() {
        let value = serde_json::to_value(MutationResult {
            operation_id: "op-1".into(),
            status: MutationStatus::Applied,
            entity_kind: "memo_state".into(),
            entity_id: "memo-1".into(),
            revision: 1,
            recorded_at: "2026-08-25T00:00:00Z".into(),
        })
        .unwrap();

        assert_eq!(value["operationId"], "op-1");
        assert_eq!(value["recordedAt"], "2026-08-25T00:00:00Z");
        assert!(value.get("recorded_at").is_none());
    }
}
