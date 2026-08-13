//! Lineage の中核ライブラリ。
//!
//! minos（クイック入力）・agentos（自動化の実行）・fullos（Tauri シェル）が
//! このクレートを共有する。とくに lineage(links) への追記は必ずここを通す。
//! 実装が3か所に散ると hash-chain の計算がアプリごとに分岐しうるため
//! （docs/concept/MINIMAL_ARCHITECTURE.md「4. Lineage の真正性担保」）。
//!
//! 依存方向は infrastructure → application → domain。domain は何にも依存しない。

pub mod application;
pub mod domain;
pub mod infrastructure;
