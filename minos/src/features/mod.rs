//! 機能ごとの presentation 層（画面と、その画面のためのサービス）。
//!
//! ここは `Services`（composition root）越しにユースケースを呼ぶだけで、
//! SQL も hash-chain も知らない。

pub mod capture;
pub mod window;
