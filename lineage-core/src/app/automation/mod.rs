//! 自動化（ルール＋記録 → 生成AI → 結果 document ＋ lineage）。
//!
//! - `run` … 実行の入口すべて（手動・メタ情報マッチ・スケジュール・ブラウザ方式の確定）
//! - `schedule` … cron 式による発火判定
//! - `backend` … 実行環境ごとに使えるバックエンドの線引き

pub mod backend;
pub mod run;
pub mod schedule;

#[cfg(test)]
mod test_support;

pub use backend::reject_browser_backend;
pub use run::Automation;
