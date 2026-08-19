//! minos 固有のインフラ層。
//!
//! ドメイン・ユースケース・永続化は lineage-core が持つ。ここに残るのは
//! トレイ常駐やフォアグラウンド取得のような、OS に直接触る処理だけ。

pub mod logging;
pub mod system;
