//! 設定の保存。
//!
//! minos（トレイメニュー）と fullos（設定画面）が同じ行を書く。

use anyhow::Result;

use crate::domain::ports::SettingsRepository;
use crate::domain::settings::Settings;
use crate::domain::shared::Clock;

pub struct SaveSettings<'a> {
    repository: &'a dyn SettingsRepository,
    clock: &'a dyn Clock,
}

impl<'a> SaveSettings<'a> {
    pub fn new(repository: &'a dyn SettingsRepository, clock: &'a dyn Clock) -> Self {
        Self { repository, clock }
    }

    pub fn execute(&self, workspace_id: &str, settings: Settings) -> Result<()> {
        let now = self.clock.now_rfc3339();
        for (key, value) in settings.to_entries() {
            self.repository.set(workspace_id, key, &value, &now)?;
        }
        Ok(())
    }
}
