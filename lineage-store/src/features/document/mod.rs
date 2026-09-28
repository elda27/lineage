//! アプリが準備した共有記録を、lineage と同じトランザクションで確定する。
mod commit_automation_result;
mod save_document;

pub use commit_automation_result::CommitAutomationResult;
pub use save_document::{
    LinkedDocument, SaveDocument, SaveDocumentInput, SaveDocumentOutput, WriteMode,
};

#[cfg(test)]
mod tests;
