//! Minos のカーソル・入力確定規則。
use lineage_core::domain::meta::{MetaAssignment, is_tag_terminator, parse_meta_tags};

/// 空白で終端した `#タグ` を本文から切り出す。
///
/// 返すのは `(タグを外した本文, 切り出したタグ)`。
///
/// 終端の空白がまだ無いトークンは入力途中なので本文に残す。ここで確定させると、
/// `#` の次の1文字でバッジになってしまい、2文字以上のタグが打てない。
/// `#a#b` のようなまとめ書きも切り出さず、保存時の本文解析に任せる。
pub fn split_completed_tags(body: &str) -> (String, Vec<MetaAssignment>) {
    let mut rest = String::with_capacity(body.len());
    let mut tags: Vec<MetaAssignment> = Vec::new();

    for token in body.split_inclusive(char::is_whitespace) {
        let text = token.trim_end_matches(char::is_whitespace);
        let completed = text.len() < token.len();

        let parsed = if completed && text.starts_with('#') {
            parse_meta_tags(text)
        } else {
            Vec::new()
        };

        // 1件だけ取れたときが「タグ1つを打ち終えた」状態。それ以外は本文のまま。
        if parsed.len() != 1 {
            rest.push_str(token);
            continue;
        }
        tags.extend(parsed);
    }

    (rest, tags)
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

#[cfg(test)]
mod tests {
    use super::*;
    /// 入力途中のタグを確定させてしまうと、2文字目が打てなくなる。
    #[test]
    fn keeps_a_tag_that_is_still_being_typed() {
        for typing in ["#", "#タ", "#タス", "メモ #タス", "#銘柄=SO"] {
            let (rest, tags) = split_completed_tags(typing);
            assert_eq!(rest, typing, "{typing} を書き換えてはいけない");
            assert!(tags.is_empty(), "{typing} を確定させてはいけない");
        }
    }

    #[test]
    fn promotes_a_tag_once_whitespace_ends_it() {
        let (rest, tags) = split_completed_tags("#タスク メモ");
        assert_eq!(rest, "メモ");
        assert_eq!(tags, vec![MetaAssignment::user("タスク", None)]);

        let (rest, tags) = split_completed_tags("SOXL #銘柄=SOXL 損切り");
        assert_eq!(rest, "SOXL 損切り");
        assert_eq!(
            tags,
            vec![MetaAssignment::user("銘柄", Some("SOXL".into()))]
        );

        // 改行も終端になる。
        let (rest, tags) = split_completed_tags("#タスク\n");
        assert_eq!(rest, "");
        assert_eq!(tags, vec![MetaAssignment::user("タスク", None)]);
    }

    #[test]
    fn leaves_plain_text_and_bundled_tags_in_the_body() {
        let (rest, tags) = split_completed_tags("ただの本文 です");
        assert_eq!(rest, "ただの本文 です");
        assert!(tags.is_empty());

        // まとめ書きは保存時の本文解析に任せる。
        let (rest, tags) = split_completed_tags("#a#b ");
        assert_eq!(rest, "#a#b ");
        assert!(tags.is_empty());

        // `#` だけのトークンはタグにならない。
        let (rest, tags) = split_completed_tags("色は # ");
        assert_eq!(rest, "色は # ");
        assert!(tags.is_empty());
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
}
