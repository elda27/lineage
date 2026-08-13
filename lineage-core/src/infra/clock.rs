//! `Clock` / `IdGenerator` port の実装。

#[cfg(any(test, feature = "testing"))]
use std::cell::Cell;

use chrono::SecondsFormat;

use crate::domain::shared::{Clock, IdGenerator};

/// システム時刻（UTC, RFC3339 秒精度）。
pub struct SystemClock;

impl Clock for SystemClock {
    fn now_rfc3339(&self) -> String {
        chrono::Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true)
    }
}

/// UUID v4。
pub struct UuidGenerator;

impl IdGenerator for UuidGenerator {
    fn new_id(&self) -> String {
        uuid::Uuid::new_v4().to_string()
    }
}

/// テスト用の固定時計。
#[cfg(any(test, feature = "testing"))]
pub struct FixedClock {
    now: String,
}

#[cfg(any(test, feature = "testing"))]
impl FixedClock {
    pub fn new(now: impl Into<String>) -> Self {
        Self { now: now.into() }
    }
}

#[cfg(any(test, feature = "testing"))]
impl Clock for FixedClock {
    fn now_rfc3339(&self) -> String {
        self.now.clone()
    }
}

/// テスト用の連番 ID。
#[cfg(any(test, feature = "testing"))]
pub struct SequentialIds {
    next: Cell<u64>,
}

#[cfg(any(test, feature = "testing"))]
impl SequentialIds {
    pub fn new() -> Self {
        Self { next: Cell::new(1) }
    }
}

#[cfg(any(test, feature = "testing"))]
impl Default for SequentialIds {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(any(test, feature = "testing"))]
impl IdGenerator for SequentialIds {
    fn new_id(&self) -> String {
        let id = self.next.get();
        self.next.set(id + 1);
        format!("id-{id:04}")
    }
}
