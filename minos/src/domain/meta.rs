//! メタ情報（`#タグ`）とその入力補完のルール。
//!
//! docs/ui.md「minos」2. に対応する。
//!
//! - メタ情報には「自動追加」と「ユーザ入力」の2種類がある
//! - ユーザ入力は過去の入力を学習した候補から補完する
//! - 短縮文字列(shorthand)を定義すると、その先頭一致でも候補に出る
//!   （`#タスク` に `task` を設定 → `#t` で候補、`#ta` でさらに絞り込み）

/// メタ情報がどこから来たか。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MetaSource {
    /// minos が自動的に付けた（直前のアプリ情報など）。
    Auto,
    /// ユーザが本文に `#` で書いた。
    User,
}

impl MetaSource {
    pub fn as_str(self) -> &'static str {
        match self {
            MetaSource::Auto => "auto",
            MetaSource::User => "user",
        }
    }
}

/// 1件の記録に付与されたメタ情報。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MetaAssignment {
    pub label: String,
    pub value: Option<String>,
    pub source: MetaSource,
}

impl MetaAssignment {
    pub fn auto(label: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            value: Some(value.into()),
            source: MetaSource::Auto,
        }
    }

    pub fn user(label: impl Into<String>, value: Option<String>) -> Self {
        Self {
            label: label.into(),
            value,
            source: MetaSource::User,
        }
    }
}

/// 学習済みのメタ情報タグ。補完候補の母集合になる。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MetaTag {
    pub id: String,
    pub workspace_id: String,
    pub label: String,
    /// fullos で設定する短縮文字列。未設定なら `None`。
    pub shorthand: Option<String>,
    pub usage_count: i64,
    pub last_used_at: Option<String>,
}

/// 補完候補が「どう一致したか」。並び順の決定と、候補一覧の説明表示に使う。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum MatchKind {
    /// ラベルの先頭一致（`#タ` → `#タスク`）。
    LabelPrefix,
    /// 短縮文字列の先頭一致（`#t` → `#タスク`）。
    ShorthandPrefix,
    /// ラベルの部分一致。
    LabelContains,
}

/// 並び替え済みの補完候補。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MetaSuggestion {
    pub label: String,
    pub shorthand: Option<String>,
    pub usage_count: i64,
    pub matched: MatchKind,
}

/// 自動付与するメタ情報のラベル。
pub mod auto_label {
    /// 直前に開いていたアプリケーションの実行ファイル名。
    pub const APP: &str = "app";
    /// 直前に開いていたウィンドウのタイトル。
    pub const WINDOW: &str = "window";
}

/// 本文中の `#ラベル` / `#ラベル=値` を取り出す。
///
/// ラベルは空白・改行・`#` で終端する。同じラベルが複数回現れた場合は最初の1件だけを採る。
pub fn parse_meta_tags(body: &str) -> Vec<MetaAssignment> {
    let mut found: Vec<MetaAssignment> = Vec::new();
    let chars: Vec<char> = body.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        if chars[i] != '#' {
            i += 1;
            continue;
        }

        let mut j = i + 1;
        while j < chars.len() && !is_tag_terminator(chars[j]) {
            j += 1;
        }

        let token: String = chars[i + 1..j].iter().collect();
        i = j;

        let Some((label, value)) = split_label_value(&token) else {
            continue;
        };
        if found.iter().any(|m| m.label == label) {
            continue;
        }
        found.push(MetaAssignment::user(label, value));
    }

    found
}

/// カーソル位置から遡って、補完対象になっている `#` トークンを探す。
///
/// 返すのは `(`#` のバイト位置, `#` を除いたクエリ文字列)`。
/// カーソルが `#` トークンの中にいない場合は `None`。
pub fn find_active_tag_token(text_before_cursor: &str) -> Option<(usize, String)> {
    for (byte_index, c) in text_before_cursor.char_indices().rev() {
        if c == '#' {
            // '#' は1バイトなので、この加算は文字境界を割らない。
            return Some((byte_index, text_before_cursor[byte_index + 1..].to_string()));
        }
        if is_tag_terminator(c) {
            return None;
        }
    }

    None
}

/// 学習済みタグから、クエリに対する補完候補を順位付きで返す。
///
/// `query` は `#` を除いた入力（空文字なら「よく使う順」）。
pub fn rank_candidates(tags: &[MetaTag], query: &str, limit: usize) -> Vec<MetaSuggestion> {
    let needle = query.trim().to_lowercase();

    let mut matched: Vec<MetaSuggestion> = tags
        .iter()
        .filter_map(|tag| match_kind(tag, &needle).map(|matched| MetaSuggestion {
            label: tag.label.clone(),
            shorthand: tag.shorthand.clone(),
            usage_count: tag.usage_count,
            matched,
        }))
        .collect();

    matched.sort_by(|a, b| {
        a.matched
            .cmp(&b.matched)
            .then(b.usage_count.cmp(&a.usage_count))
            .then(a.label.cmp(&b.label))
    });
    matched.truncate(limit);
    matched
}

fn match_kind(tag: &MetaTag, needle: &str) -> Option<MatchKind> {
    if needle.is_empty() {
        return Some(MatchKind::LabelPrefix);
    }

    let label = tag.label.to_lowercase();
    if label.starts_with(needle) {
        return Some(MatchKind::LabelPrefix);
    }
    if let Some(shorthand) = &tag.shorthand
        && shorthand.to_lowercase().starts_with(needle)
    {
        return Some(MatchKind::ShorthandPrefix);
    }
    if label.contains(needle) {
        return Some(MatchKind::LabelContains);
    }
    None
}

fn is_tag_terminator(c: char) -> bool {
    c.is_whitespace() || c == '#' || c == ',' || c == '、' || c == '　'
}

fn split_label_value(token: &str) -> Option<(String, Option<String>)> {
    let token = token.trim();
    if token.is_empty() {
        return None;
    }
    match token.split_once('=') {
        Some((label, value)) if !label.is_empty() => {
            let value = value.trim();
            Some((
                label.to_string(),
                (!value.is_empty()).then(|| value.to_string()),
            ))
        }
        _ => Some((token.to_string(), None)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tag(label: &str, shorthand: Option<&str>, usage: i64) -> MetaTag {
        MetaTag {
            id: format!("tag-{label}"),
            workspace_id: "ws".into(),
            label: label.into(),
            shorthand: shorthand.map(str::to_string),
            usage_count: usage,
            last_used_at: None,
        }
    }

    #[test]
    fn parses_labels_and_values() {
        let metas = parse_meta_tags("SOXL 損切り #タスク #銘柄=SOXL のメモ");
        assert_eq!(
            metas,
            vec![
                MetaAssignment::user("タスク", None),
                MetaAssignment::user("銘柄", Some("SOXL".into())),
            ]
        );
    }

    #[test]
    fn parses_adjacent_tags_and_deduplicates() {
        let metas = parse_meta_tags("#a#b #a");
        assert_eq!(
            metas.iter().map(|m| m.label.as_str()).collect::<Vec<_>>(),
            vec!["a", "b"]
        );
    }

    #[test]
    fn ignores_a_bare_hash() {
        assert!(parse_meta_tags("# ").is_empty());
        assert!(parse_meta_tags("色は #").is_empty());
    }

    #[test]
    fn finds_the_token_under_the_cursor() {
        // "メモ " は 7 バイト。返るのは '#' のバイト位置。
        assert_eq!(
            find_active_tag_token("メモ #タス"),
            Some((7, "タス".to_string()))
        );
        assert_eq!(find_active_tag_token("メモ #"), Some((7, String::new())));
        assert_eq!(find_active_tag_token("メモ タス"), None);
        assert_eq!(find_active_tag_token(""), None);
    }

    #[test]
    fn shorthand_prefix_matches_and_narrows() {
        let tags = vec![
            tag("タスク", Some("task"), 10),
            tag("タイムライン", Some("timeline"), 3),
            tag("投資", Some("trade"), 1),
        ];

        let all_t: Vec<_> = rank_candidates(&tags, "t", 10)
            .into_iter()
            .map(|s| s.label)
            .collect();
        assert_eq!(all_t, vec!["タスク", "タイムライン", "投資"]);

        let narrowed: Vec<_> = rank_candidates(&tags, "ta", 10)
            .into_iter()
            .map(|s| s.label)
            .collect();
        assert_eq!(narrowed, vec!["タスク"]);
    }

    #[test]
    fn label_prefix_wins_over_shorthand() {
        let tags = vec![tag("task-b", None, 1), tag("あ", Some("task-a"), 100)];
        let ranked = rank_candidates(&tags, "task", 10);
        assert_eq!(ranked[0].label, "task-b");
        assert_eq!(ranked[0].matched, MatchKind::LabelPrefix);
        assert_eq!(ranked[1].matched, MatchKind::ShorthandPrefix);
    }

    #[test]
    fn empty_query_returns_most_used_first() {
        let tags = vec![tag("低", None, 1), tag("高", None, 50)];
        let ranked = rank_candidates(&tags, "", 10);
        assert_eq!(ranked[0].label, "高");
    }
}
