//! Lineage（hash-chain）の検証。
//!
//! 鎖への**追記**は記録を生む側（`capture` / `automation`）が
//! それぞれのトランザクションの中で行う。ここは読み出して検証するだけ。

pub mod verify_lineage;

pub use verify_lineage::VerifyLineage;
