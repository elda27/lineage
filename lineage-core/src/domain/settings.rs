//! 利用者の設定。
//!
//! 保存形式は `settings(workspace_id, key, value)` の文字列だが、
//! 「どのキーがあるか」「既定値は何か」「文字列をどう解釈するか」は方針なのでドメインに置く。

/// 設定キー。fullos からも同じキーを読む。
pub mod key {
    /// Alt+Space で呼び出したとき、直前のアプリの選択テキストを自動で取り込むか。
    pub const AUTO_PULL_FOREGROUND_TEXT: &str = "minos.auto_pull_foreground_text";
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Settings {
    /// 自動取り込み。有効にすると呼び出しのたびに直前のアプリへ Ctrl+C を送り、
    /// クリップボードを一時使用してから元の内容へ戻す。
    pub auto_pull_foreground_text: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            auto_pull_foreground_text: true,
        }
    }
}

impl Settings {
    /// 保存済みの `(key, value)` から組み立てる。未知のキーと壊れた値は既定値で埋める。
    pub fn from_entries(entries: &[(String, String)]) -> Self {
        let mut settings = Self::default();
        for (key, value) in entries {
            if key == key::AUTO_PULL_FOREGROUND_TEXT
                && let Some(parsed) = parse_bool(value)
            {
                settings.auto_pull_foreground_text = parsed;
            }
        }
        settings
    }

    /// 保存する `(key, value)` の一覧。
    pub fn to_entries(self) -> Vec<(&'static str, String)> {
        vec![(
            key::AUTO_PULL_FOREGROUND_TEXT,
            format_bool(self.auto_pull_foreground_text),
        )]
    }
}

fn parse_bool(value: &str) -> Option<bool> {
    match value.trim() {
        "true" | "1" => Some(true),
        "false" | "0" => Some(false),
        _ => None,
    }
}

fn format_bool(value: bool) -> String {
    if value { "true" } else { "false" }.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_to_pulling_the_selection() {
        assert!(Settings::default().auto_pull_foreground_text);
    }

    #[test]
    fn reads_a_stored_value() {
        let entries = vec![(
            key::AUTO_PULL_FOREGROUND_TEXT.to_string(),
            "false".to_string(),
        )];
        assert!(!Settings::from_entries(&entries).auto_pull_foreground_text);
    }

    #[test]
    fn falls_back_to_the_default_for_unusable_values() {
        let entries = vec![(
            key::AUTO_PULL_FOREGROUND_TEXT.to_string(),
            "maybe".to_string(),
        )];
        assert!(Settings::from_entries(&entries).auto_pull_foreground_text);
        assert!(Settings::from_entries(&[]).auto_pull_foreground_text);
    }

    #[test]
    fn round_trips_through_entries() {
        let settings = Settings {
            auto_pull_foreground_text: false,
        };
        let entries: Vec<(String, String)> = settings
            .to_entries()
            .into_iter()
            .map(|(key, value)| (key.to_string(), value))
            .collect();
        assert_eq!(Settings::from_entries(&entries), settings);
    }
}
