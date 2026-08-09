//! 設定の読み書き。

use anyhow::Result;

use crate::domain::ports::SettingsRepository;
use crate::domain::settings::Settings;
use crate::domain::shared::Clock;

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::clock::FixedClock;
    use crate::infrastructure::sqlite::Database;

    #[test]
    fn round_trips_through_the_database() {
        let db = Database::open_in_memory().unwrap();
        let clock = FixedClock::new("2026-08-09T00:00:00Z");

        // 未保存なら既定値。
        assert_eq!(LoadSettings::new(&db).execute("ws").unwrap(), Settings::default());

        let settings = Settings {
            auto_pull_foreground_text: false,
        };
        SaveSettings::new(&db, &clock).execute("ws", settings).unwrap();
        assert_eq!(LoadSettings::new(&db).execute("ws").unwrap(), settings);

        // 上書きできる（PRIMARY KEY の重複で失敗しない）。
        SaveSettings::new(&db, &clock)
            .execute("ws", Settings::default())
            .unwrap();
        assert_eq!(LoadSettings::new(&db).execute("ws").unwrap(), Settings::default());
    }
}
