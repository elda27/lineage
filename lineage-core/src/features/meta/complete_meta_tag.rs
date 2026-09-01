//! `#` を押したときの入力補完。
//!
//! 候補は「過去にユーザが入力したメタ情報」を学習した `meta_tags` から作る。
//! 並び順のルールは domain（`meta::rank_candidates`）に置き、
//! ここは「読み出して並べる」だけにする。

use anyhow::Result;

use crate::domain::meta::{MetaSuggestion, rank_candidates};
use crate::domain::ports::MetaTagQuery;

/// 補完候補として読み出す学習済みタグの上限。
const TAG_POOL_LIMIT: usize = 500;

pub struct CompleteMetaTag<'a> {
    tags: &'a dyn MetaTagQuery,
}

impl<'a> CompleteMetaTag<'a> {
    pub fn new(tags: &'a dyn MetaTagQuery) -> Self {
        Self { tags }
    }

    /// `query` は `#` を除いた入力文字列。空なら「よく使う順」。
    pub fn execute(
        &self,
        workspace_id: &str,
        query: &str,
        limit: usize,
    ) -> Result<Vec<MetaSuggestion>> {
        let tags = self.tags.all(workspace_id, TAG_POOL_LIMIT)?;
        Ok(rank_candidates(&tags, query, limit))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::meta::MetaTag;

    struct StubTags(Vec<MetaTag>);

    impl MetaTagQuery for StubTags {
        fn all(&self, _workspace_id: &str, _limit: usize) -> Result<Vec<MetaTag>> {
            Ok(self.0.clone())
        }
    }

    fn tag(label: &str, shorthand: Option<&str>, usage: i64) -> MetaTag {
        MetaTag {
            id: label.into(),
            workspace_id: "ws".into(),
            label: label.into(),
            shorthand: shorthand.map(str::to_string),
            usage_count: usage,
            last_used_at: None,
        }
    }

    #[test]
    fn suggests_by_shorthand_prefix() {
        let tags = StubTags(vec![
            tag("タスク", Some("task"), 5),
            tag("投資", Some("trade"), 2),
        ]);

        let suggestions = CompleteMetaTag::new(&tags).execute("ws", "t", 10).unwrap();
        assert_eq!(
            suggestions
                .iter()
                .map(|s| s.label.as_str())
                .collect::<Vec<_>>(),
            vec!["タスク", "投資"]
        );
    }

    #[test]
    fn honours_the_limit() {
        let tags = StubTags(vec![
            tag("a", None, 3),
            tag("b", None, 2),
            tag("c", None, 1),
        ]);
        assert_eq!(
            CompleteMetaTag::new(&tags)
                .execute("ws", "", 2)
                .unwrap()
                .len(),
            2
        );
    }
}
