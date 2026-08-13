//! インフラ層。domain の port を具体的な技術で実装する。
//!
//! OS 固有の処理（トレイ・ウィンドウ・フォアグラウンド取得）はここには置かない。
//! それは minos 側の `infra/system` に残す。

pub mod anthropic;
pub mod clock;
pub mod credentials;
pub mod crypto;
pub mod sqlite;
