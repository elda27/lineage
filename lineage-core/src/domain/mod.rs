//! ドメイン層。
//!
//! この層は他のどの層にも依存しない（infrastructure / presentation を参照しない）。
//! DB・Win32・gpui の API はここには一切現れない。

pub mod automation;
pub mod capture;
pub mod lineage;
pub mod meta;
pub mod mutation;
pub mod ports;
pub mod settings;
pub mod shared;
pub mod tag;
