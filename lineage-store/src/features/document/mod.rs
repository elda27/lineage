//! アプリが準備した共有記録を、lineage と同じトランザクションで確定する。
mod save_document;

pub use save_document::{
    LinkedDocument, SaveDocument, SaveDocumentInput, SaveDocumentOutput, WriteMode,
};
