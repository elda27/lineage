//! Minos のファイル保存・OS 連携。
pub mod attachments;
#[cfg(feature = "desktop")]
pub mod logging;
pub mod storage;
#[cfg(feature = "desktop")]
pub mod system;
