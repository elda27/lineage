//! `#` を押したときの入力補完を gpui-component の入力欄に繋ぐ。
//!
//! 候補の中身と並び順は application/domain 側が決める。
//! ここが担うのは「カーソル位置から補完対象のトークンを切り出し、
//! 置き換え範囲つきの補完項目に変換する」ことだけ。

use std::rc::Rc;

use anyhow::Result;
use gpui::{Context, Task, Window};
use gpui_component::input::{CompletionProvider, InputState, Rope, RopeExt};
use lsp_types::{
    CompletionContext, CompletionItem, CompletionItemKind, CompletionResponse, CompletionTextEdit,
    Range as LspRange, TextEdit,
};

use lineage_core::domain::meta::{MatchKind, find_active_tag_token};

use crate::app::Services;

/// 一度に出す候補の上限。
const MAX_SUGGESTIONS: usize = 12;

pub struct MetaCompletionProvider {
    services: Rc<Services>,
}

impl MetaCompletionProvider {
    pub fn new(services: Rc<Services>) -> Self {
        Self { services }
    }
}

impl CompletionProvider for MetaCompletionProvider {
    fn completions(
        &self,
        text: &Rope,
        offset: usize,
        _trigger: CompletionContext,
        _window: &mut Window,
        _cx: &mut Context<InputState>,
    ) -> Task<Result<CompletionResponse>> {
        let before_cursor = text.slice(..offset).to_string();
        let Some((token_start, query)) = find_active_tag_token(&before_cursor) else {
            return Task::ready(Ok(CompletionResponse::Array(vec![])));
        };

        let suggestions = match self.services.suggest_meta_tags(&query, MAX_SUGGESTIONS) {
            Ok(suggestions) => suggestions,
            Err(error) => {
                log::error!("メタ情報の候補を取得できません: {error:#}");
                return Task::ready(Ok(CompletionResponse::Array(vec![])));
            }
        };

        // 置き換えるのは `#` からカーソルまで。入力済みの `#タス` が `#タスク ` になる。
        let range = LspRange {
            start: text.offset_to_position(token_start),
            end: text.offset_to_position(offset),
        };

        let items = suggestions
            .into_iter()
            .map(|suggestion| CompletionItem {
                label: format!("#{}", suggestion.label),
                kind: Some(CompletionItemKind::KEYWORD),
                detail: Some(detail_of(&suggestion.shorthand, suggestion.usage_count, suggestion.matched)),
                filter_text: Some(format!("#{}", suggestion.label)),
                text_edit: Some(CompletionTextEdit::Edit(TextEdit {
                    range,
                    new_text: format!("#{} ", suggestion.label),
                })),
                ..Default::default()
            })
            .collect();

        Task::ready(Ok(CompletionResponse::Array(items)))
    }

    fn is_completion_trigger(
        &self,
        _offset: usize,
        new_text: &str,
        _cx: &mut Context<InputState>,
    ) -> bool {
        // 空白・改行を打った時点でタグは終わっている。
        // それ以外は毎回問い合わせ、`#` トークンの中でなければ `completions` が空を返す。
        !new_text.is_empty() && !new_text.chars().any(char::is_whitespace)
    }
}

fn detail_of(shorthand: &Option<String>, usage_count: i64, matched: MatchKind) -> String {
    let mut detail = match shorthand {
        Some(shorthand) => format!("{shorthand} · {usage_count}回"),
        None => format!("{usage_count}回"),
    };
    if matched == MatchKind::ShorthandPrefix {
        detail.push_str(" · 短縮");
    }
    detail
}
