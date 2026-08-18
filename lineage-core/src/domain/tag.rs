//! Stable tag registry and its view/recipe bindings.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TagKind {
    User,
    Builtin,
    Metadata,
}

impl TagKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Builtin => "builtin",
            Self::Metadata => "metadata",
        }
    }
    pub fn parse(value: &str) -> Self {
        match value {
            "builtin" => Self::Builtin,
            "metadata" => Self::Metadata,
            _ => Self::User,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TagAssignmentSource {
    User,
    Derived,
    Imported,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ViewBinding {
    pub tag_id: String,
    pub view_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AutomationBinding {
    pub tag_id: String,
    pub recipe_name: String,
    pub managed: bool,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TagDefinition {
    pub id: String,
    pub workspace_id: String,
    pub kind: TagKind,
    pub display_name: String,
    pub shorthand: Option<String>,
    pub usage_count: i64,
    pub last_used_at: Option<String>,
    pub enabled: bool,
    pub managed: bool,
    pub deleted_at: Option<String>,
    pub view: Option<ViewBinding>,
    pub automation: Option<AutomationBinding>,
}
