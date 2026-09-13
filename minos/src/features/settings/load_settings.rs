//! 設定の読み出し。

use anyhow::Result;

use crate::domain::settings::Settings;
use lineage_core::domain::ports::SettingsRepository;

pub struct LoadSettings<'a> {
    repository: &'a dyn SettingsRepository,
}

impl<'a> LoadSettings<'a> {
    pub fn new(repository: &'a dyn SettingsRepository) -> Self {
        Self { repository }
    }

    pub fn execute(&self, workspace_id: &str) -> Result<Settings> {
        Ok(Settings::from_entries(&self.repository.all(workspace_id)?))
    }
}
