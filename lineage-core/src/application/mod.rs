//! アプリケーション層（ユースケース）。
//!
//! port(interface) だけに依存し、具体的な DB 実装や Win32 API には依存しない。

pub mod capture_memo;
pub mod complete_meta_tag;
pub mod settings;
pub mod verify_lineage;
