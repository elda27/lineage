//! 入力 UI の規則を持たず、記録・タグ・観測メタデータ・添付・lineage を一括保存する。

use crate::domain::document::DocumentAsset;
use crate::domain::lineage::{LineageInput, LineageLedger, relation};
use crate::domain::meta::{DocumentMetadata, MetaAssignment};
use crate::domain::ports::{DocumentStore, DocumentTx};
use crate::domain::shared::{Hasher, IdGenerator};
use anyhow::{Result, ensure};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WriteMode {
    Insert,
    Update,
}

/// 添付記録と親記録に向かう来歴。source・actor の選択は呼び出すアプリが行う。
pub struct LinkedDocument {
    pub document: DocumentAsset,
    pub lineage: LineageInput,
}

pub struct SaveDocumentInput {
    pub workspace_name: String,
    pub document: DocumentAsset,
    pub mode: WriteMode,
    pub lineage: LineageInput,
    pub metas: Vec<MetaAssignment>,
    pub metadata: Vec<DocumentMetadata>,
    pub attachments: Vec<LinkedDocument>,
}

pub struct SaveDocumentOutput {
    pub seq: i64,
    pub content_hash: String,
}

pub struct SaveDocument<'a> {
    store: &'a dyn DocumentStore,
    ids: &'a dyn IdGenerator,
    hasher: &'a dyn Hasher,
}

impl<'a> SaveDocument<'a> {
    pub fn new(
        store: &'a dyn DocumentStore,
        ids: &'a dyn IdGenerator,
        hasher: &'a dyn Hasher,
    ) -> Self {
        Self { store, ids, hasher }
    }

    pub fn execute(&self, input: SaveDocumentInput) -> Result<SaveDocumentOutput> {
        let document = &input.document;
        validate_target(document, &input.lineage)?;
        for attachment in &input.attachments {
            validate_target(document, &attachment.lineage)?;
            ensure!(
                attachment.document.workspace_id == document.workspace_id,
                "添付記録の workspace が一致しません"
            );
            ensure!(
                attachment.lineage.source_kind == "document"
                    && attachment.lineage.source_id == attachment.document.id
                    && attachment.lineage.relation_type == relation::ATTACHMENT_FOR,
                "添付記録と lineage が一致しません"
            );
        }
        let now = &document.updated_at;
        let mut appended = None;
        self.store.transact(&mut |tx: &mut dyn DocumentTx| {
            tx.ensure_workspace(&document.workspace_id, &input.workspace_name, now)?;
            match input.mode {
                WriteMode::Insert => tx.insert_document(document)?,
                WriteMode::Update => {
                    tx.update_document(document)?;
                    tx.clear_document_metas(&document.id)?;
                }
            }
            for metadata in &input.metadata {
                tx.insert_document_metadata(&self.ids.new_id(), &document.id, metadata, now)?;
            }
            for meta in &input.metas {
                tx.learn_meta_tag(&self.ids.new_id(), &document.workspace_id, &meta.label, now)?;
                tx.insert_document_meta(&self.ids.new_id(), &document.id, meta, now)?;
            }
            let previous = tx.last_link(&document.workspace_id)?;
            let ledger = LineageLedger::new(self.hasher);
            let mut link =
                ledger.append_next(previous.as_ref(), self.ids.new_id(), input.lineage.clone());
            tx.append_link(&link)?;
            for attachment in &input.attachments {
                tx.insert_document(&attachment.document)?;
                link =
                    ledger.append_next(Some(&link), self.ids.new_id(), attachment.lineage.clone());
                tx.append_link(&link)?;
            }
            appended = Some(SaveDocumentOutput {
                seq: link.seq,
                content_hash: link.content_hash,
            });
            Ok(())
        })?;
        appended.ok_or_else(|| anyhow::anyhow!("lineage が追記されませんでした"))
    }
}

pub(super) fn validate_target(document: &DocumentAsset, lineage: &LineageInput) -> Result<()> {
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
