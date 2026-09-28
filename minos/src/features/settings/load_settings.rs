//! 設定の読み出し。

use anyhow::Result;

use crate::domain::settings::Settings;
use lineage_store::ports::SettingsRepository;

pub struct LoadSettings<'a> {
    repository: &'a dyn SettingsRepository,
}

impl<'a> LoadSettings<'a> {
    pub fn new(repository: &'a dyn SettingsRepository) -> Self {
        Self { repository }
    }

    pub fn execute(&self, workspace_id: &str) -> Result<Settings> {
        Settings::from_entries(&self.repository.all(workspace_id)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct InvalidSettings;

    impl SettingsRepository for InvalidSettings {
        fn all(&self, _workspace_id: &str) -> Result<Vec<(String, String)>> {
            Ok(vec![(
                crate::domain::settings::key::AUTO_PULL_FOREGROUND_TEXT.to_string(),
                "maybe".to_string(),
            )])
        }
    }

    #[test]
    fn propagates_invalid_persisted_settings() {
        assert!(LoadSettings::new(&InvalidSettings).execute("ws").is_err());
    }
}
