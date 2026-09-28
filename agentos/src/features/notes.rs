//! Runner-owned data CLI use cases; persistence owns only the atomic write.
use anyhow::{Result, ensure};
use lineage_core::domain::{
    document::{DOCUMENT_TYPE_MEMO, DocumentAsset},
    lineage::{LineageInput, relation},
};
use lineage_store::{
    features::document::{SaveDocument, SaveDocumentInput, WriteMode},
    infra::{
        clock::{SystemClock, UuidGenerator},
        crypto::Sha256Hasher,
        sqlite::Database,
    },
    ports::{Clock, DocumentQuery, IdGenerator},
};
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PutNote {
    pub id: Option<String>,
    pub title: String,
    pub body_text: String,
}

pub fn put(db: &Database, workspace: &str, input: PutNote) -> Result<String> {
    ensure!(!input.title.trim().is_empty(), "title is required");
    let previous = match &input.id {
        Some(id) => Some(
            DocumentQuery::get(db, workspace, id)?
                .ok_or_else(|| anyhow::anyhow!("note not found in workspace"))?,
        ),
        None => None,
    };
    let id = input.id.unwrap_or_else(|| UuidGenerator.new_id());
    let now = SystemClock.now_rfc3339();
    SaveDocument::new(db, &UuidGenerator, &Sha256Hasher).execute(SaveDocumentInput {
        workspace_name: workspace.into(),
        mode: if previous.is_some() {
            WriteMode::Update
        } else {
            WriteMode::Insert
        },
        document: DocumentAsset {
            id: id.clone(),
            workspace_id: workspace.into(),
            title: input.title,
            body_text: input.body_text,
            blob_uri: None,
            document_type: DOCUMENT_TYPE_MEMO.into(),
            created_at: previous
                .as_ref()
                .map(|v| v.created_at.clone())
                .unwrap_or_else(|| now.clone()),
            updated_at: now.clone(),
        },
        lineage: LineageInput {
            workspace_id: workspace.into(),
            source_kind: "cli".into(),
            source_id: "put".into(),
            target_kind: "document".into(),
            target_id: id.clone(),
            relation_type: relation::DERIVED_FROM.into(),
            actor: "user:cli".into(),
            created_at: now,
        },
        metas: previous.map(|v| v.metas).unwrap_or_default(),
        metadata: vec![],
        attachments: vec![],
    })?;
    Ok(id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use lineage_store::ports::LineageQuery;
    #[test]
    fn create_update_and_workspace_isolation_preserve_chain() {
        let db = Database::open_in_memory().unwrap();
        let input = |id, body: &str| PutNote {
            id,
            title: "note".into(),
            body_text: body.into(),
        };
        let id = put(&db, "local", input(None, "first")).unwrap();
        put(&db, "local", input(Some(id.clone()), "second")).unwrap();
        assert_eq!(
            DocumentQuery::get(&db, "local", &id)
                .unwrap()
                .unwrap()
                .body_text,
            "second"
        );
        assert!(put(&db, "other", input(Some(id), "wrong workspace")).is_err());
        let links = LineageQuery::list(&db, "local").unwrap();
        assert_eq!(links.len(), 2);
        assert!(
            lineage_core::domain::lineage::LineageLedger::new(&Sha256Hasher)
                .verify(&links)
                .is_ok()
        );
    }
}
