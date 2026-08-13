//! 設定の読み書き。読みと書きは別のユースケースだが、往復で1つの振る舞いなのでテストはここに置く。

pub mod load_settings;
pub mod save_settings;

pub use load_settings::LoadSettings;
pub use save_settings::SaveSettings;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::settings::Settings;
    use crate::infra::clock::FixedClock;
    use crate::infra::sqlite::Database;

    #[test]
    fn round_trips_through_the_database() {
        let db = Database::open_in_memory().unwrap();
        let clock = FixedClock::new("2026-08-09T00:00:00Z");

        // 未保存なら既定値。
        assert_eq!(
            LoadSettings::new(&db).execute("ws").unwrap(),
            Settings::default()
        );

        let settings = Settings {
            auto_pull_foreground_text: false,
        };
        SaveSettings::new(&db, &clock)
            .execute("ws", settings)
            .unwrap();
        assert_eq!(LoadSettings::new(&db).execute("ws").unwrap(), settings);

        // 上書きできる（PRIMARY KEY の重複で失敗しない）。
        SaveSettings::new(&db, &clock)
            .execute("ws", Settings::default())
            .unwrap();
        assert_eq!(
            LoadSettings::new(&db).execute("ws").unwrap(),
            Settings::default()
        );
    }
}
