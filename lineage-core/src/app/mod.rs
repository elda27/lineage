//! アプリケーション層（ユースケース）。
//!
//! port(interface) だけに依存し、具体的な DB 実装や Win32 API には依存しない。
//!
//! アプリの接点は**機能単位**で分ける。1機能＝1フォルダ、1ユースケース＝1ファイルとし、
//! 機能の `mod.rs` がその機能の入口（ユースケース）を再輸出する。
//! 呼び出し側は `lineage_core::app::<機能>::<ユースケース>` の形だけを見ればよく、
//! ファイルの割り方が変わっても呼び出しは変わらない。
//!
//! 機能の名前は domain 側の集約（`domain::automation` ほか）と1対1に対応させる。

pub mod automation;
pub mod capture;
pub mod lineage;
pub mod meta;
pub mod mutation;
pub mod settings;
