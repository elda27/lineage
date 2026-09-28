//! クイック入力の確定と画面。
mod capture_memo;
pub mod complete_meta_tag;
pub mod tag_input;
pub use capture_memo::{CaptureMemo, CaptureMemoInput, CaptureMemoOutput};
#[cfg(feature = "desktop")]
pub mod meta_completion;
#[cfg(feature = "desktop")]
pub mod view;
