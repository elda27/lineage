//! スケジュール実行の発火判定。
//!
//! agentos は常駐せず状態も持たないので、「前回の実行開始より後に発火時刻があるか」を
//! そのつど cron 式から計算する（`Automation::due_schedules`）。

use std::str::FromStr;

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};

/// `last` より後、`now` までの間に cron の発火時刻があるか。
pub fn is_due(expression: &str, last: DateTime<Utc>, now: DateTime<Utc>) -> Result<bool> {
    let schedule = cron::Schedule::from_str(expression)
        .with_context(|| format!("cron 式を解釈できません: {expression}"))?;
    Ok(schedule
        .after(&last)
        .next()
        .is_some_and(|next| next <= now))
}

pub fn parse_time(value: &str) -> Result<DateTime<Utc>> {
    Ok(DateTime::parse_from_rfc3339(value)
        .with_context(|| format!("日時を解釈できません: {value}"))?
        .with_timezone(&Utc))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_schedule_fires_once_its_next_occurrence_has_passed() {
        let last = parse_time("2026-08-13T08:00:00Z").unwrap();
        // 毎時0分（cron クレートは秒フィールドを先頭に取る）。
        let hourly = "0 0 * * * *";

        assert!(!is_due(hourly, last, parse_time("2026-08-13T08:30:00Z").unwrap()).unwrap());
        assert!(is_due(hourly, last, parse_time("2026-08-13T09:00:00Z").unwrap()).unwrap());
        assert!(is_due(hourly, last, parse_time("2026-08-13T09:30:00Z").unwrap()).unwrap());
    }

    #[test]
    fn a_broken_cron_expression_is_reported() {
        let now = parse_time("2026-08-13T09:00:00Z").unwrap();
        assert!(is_due("まいにち", now, now).is_err());
    }
}
