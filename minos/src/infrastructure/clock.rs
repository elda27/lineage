//! `Clock` / `IdGenerator` port の実装。

#[cfg(test)]
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
#[cfg(test)]
pub struct FixedClock {
    now: String,
}

#[cfg(test)]
impl FixedClock {
    pub fn new(now: impl Into<String>) -> Self {
        Self { now: now.into() }
    }
}

#[cfg(test)]
impl Clock for FixedClock {
    fn now_rfc3339(&self) -> String {
        self.now.clone()
    }
}

/// テスト用の連番 ID。
#[cfg(test)]
pub struct SequentialIds {
    next: Cell<u64>,
}

#[cfg(test)]
impl SequentialIds {
    pub fn new() -> Self {
        Self { next: Cell::new(1) }
    }
}

#[cfg(test)]
impl Default for SequentialIds {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
impl IdGenerator for SequentialIds {
    fn new_id(&self) -> String {
        let id = self.next.get();
        self.next.set(id + 1);
        format!("id-{id:04}")
    }
}
