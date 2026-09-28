//! アプリ間で共有する記録・参照用スナップショット。入力方法やタイトルの導出規則は持たない。

use crate::domain::meta::MetaAssignment;
use serde::{Deserialize, Serialize};

pub const DOCUMENT_TYPE_MEMO: &str = "memo";
pub const DOCUMENT_TYPE_IMAGE: &str = "image";

/// 記録本体。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentAsset {
    pub id: String,
    pub workspace_id: String,
    pub title: String,
    pub body_text: String,
    pub blob_uri: Option<String>,
    pub document_type: String,
    pub created_at: String,
    pub updated_at: String,
}

/// 本文と付与タグをまとめた参照用の記録。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentSnapshot {
    pub id: String,
    pub title: String,
    pub body_text: String,
    pub metas: Vec<MetaAssignment>,
    pub created_at: String,
}

use crate::domain::lineage::LineageInput;
use anyhow::{Result, ensure};

pub fn validate_lineage_target(document: &DocumentAsset, lineage: &LineageInput) -> Result<()> {
    ensure!(
        lineage.workspace_id == document.workspace_id,
        "記録と lineage の workspace が一致しません"
    );
    ensure!(
        lineage.target_kind == "document" && lineage.target_id == document.id,
        "記録と lineage の target が一致しません"
    );
    Ok(())
}
