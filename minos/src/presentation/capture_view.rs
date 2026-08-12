//! minos の入力画面。
//!
//! docs/ui.md「minos」1.〜3. に対応する。
//!
//! - テキストボックス1つと送信ボタン1つ
//! - Ctrl+Enter で送信
//! - `#` でメタ情報を補完（候補は過去の入力から学習したもの）
//! - 直前のアプリ情報は自動メタ情報として付き、そのアプリの選択テキストも取り込める
//!   （自動で取り込むかはトレイメニューの設定で切り替える）

use std::rc::Rc;
use std::time::Duration;

use gpui::prelude::*;
use gpui::{Context, Entity, KeyDownEvent, SharedString, Subscription, Window, div, px};
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::input::{Escape, Input, InputEvent, InputState};
use gpui_component::{ActiveTheme, Sizable, h_flex, v_flex};

use crate::app::Services;
use crate::domain::capture::CaptureContext;
use crate::domain::meta::{MetaAssignment, parse_meta_tags};
use crate::infrastructure::system::foreground;
use crate::infrastructure::system::{ForegroundApp, SelectionCapture};
use crate::presentation::meta_completion::MetaCompletionProvider;
use crate::presentation::window_control::AppWindow;

/// 保存の手応えを見せてから引っ込むまでの時間。
const HIDE_AFTER_SAVE: Duration = Duration::from_millis(700);
/// 直前アプリへ Ctrl+C を送ってからクリップボードの更新を待つ上限。
const CLIPBOARD_WAIT: Duration = Duration::from_millis(400);
const CLIPBOARD_POLL: Duration = Duration::from_millis(20);

/// 選択テキストの取り込み方。
#[derive(Debug, Clone, Copy)]
enum Pull {
    /// ホットキー側が送信済み。待って結果を受け取るだけ（自動取り込み）。
    Awaiting(SelectionCapture),
    /// これから対象アプリへ切り替えて Ctrl+C を送る（明示操作）。
    Request { hwnd: isize },
}

#[derive(Clone)]
enum Status {
    Saved { title: SharedString, seq: i64 },
    Info(SharedString),
    Error(SharedString),
}

pub struct CaptureView {
    services: Rc<Services>,
    app_window: Rc<AppWindow>,
    input: Entity<InputState>,
    /// 本文から確定され、入力欄内にバッジとして表示するタグ。
    tags: Vec<MetaAssignment>,
    /// 直前にフォアグラウンドだったアプリ（Alt+Space を押した瞬間に観測したもの）。
    context: Option<ForegroundApp>,
    status: Option<Status>,
    _subscriptions: Vec<Subscription>,
}

impl CaptureView {
    pub fn new(
        services: Rc<Services>,
        app_window: Rc<AppWindow>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let input = cx.new(|cx| {
            let mut state = InputState::new(window, cx)
                .multi_line(true)
                .auto_grow(3, 10)
                .placeholder("いま気づいたことを書く（#でメタ情報、Ctrl+Enter で送信）");
            state.lsp.completion_provider = Some(Rc::new(MetaCompletionProvider::new(
                services.clone(),
            )));
            state
        });

        let subscriptions = vec![cx.subscribe_in(&input, window, {
            move |this, _, event: &InputEvent, window, cx| {
                // Ctrl+Enter（gpui-component では secondary-enter）で送信する。
                match event {
                    InputEvent::PressEnter { secondary: true, .. } => this.submit(window, cx),
                    InputEvent::Change => this.promote_completed_tags(window, cx),
                    _ => {}
                }
            }
        })];

        Self {
            services,
            app_window,
            input,
            tags: Vec::new(),
            context: None,
            status: None,
            _subscriptions: subscriptions,
        }
    }

    /// Alt+Space が押された瞬間に観測した「直前のアプリ」と、
    /// そのとき送った Ctrl+C の結果待ちを受け取る。
    pub fn set_context(
        &mut self,
        context: Option<ForegroundApp>,
        selection: Option<SelectionCapture>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if context.is_some() {
            self.context = context;
        }
        self.input.update(cx, |input, cx| input.focus(window, cx));
        cx.notify();

        if let Some(selection) = selection {
            self.pull_selection(Pull::Awaiting(selection), window, cx);
        }
    }

    /// 検証結果を画面に出す（トレイメニューから呼ばれる）。
    pub fn show_verification(&mut self, cx: &mut Context<Self>) {
        use crate::domain::lineage::VerifyResult;

        self.status = Some(match self.services.verify_lineage() {
            Ok(VerifyResult::Ok { checked }) => {
                Status::Info(format!("記録 {checked} 件の整合性を確認しました").into())
            }
            Ok(VerifyResult::Broken { broken_at, reason }) => Status::Error(
                format!("記録 {broken_at} 件目で不整合を検出しました（{reason:?}）").into(),
            ),
            Err(error) => Status::Error(format!("検証に失敗しました: {error}").into()),
        });
        cx.notify();
    }

    /// 入力を確定して保存する。
    fn submit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let body = self.input.read(cx).value().to_string();
        if body.trim().is_empty() {
            self.status = Some(Status::Error("本文が空です".into()));
            cx.notify();
            return;
        }

        let capture_context = self.context.as_ref().map(|app| CaptureContext {
            process_name: app.process_name.clone(),
            window_title: app.window_title.clone(),
        });

        match self
            .services
            .capture(body, self.tags.clone(), capture_context)
        {
            Ok(output) => {
                self.input
                    .update(cx, |input, cx| input.set_value("", window, cx));
                self.tags.clear();
                self.status = Some(Status::Saved {
                    title: output.title.into(),
                    seq: output.seq,
                });
                cx.notify();
                self.hide_after_delay(cx);
            }
            Err(error) => {
                self.status = Some(Status::Error(format!("保存できません: {error}").into()));
                cx.notify();
            }
        }
    }

    /// 空白などで確定した `#タグ` を本文から取り除き、バッジへ移す。
    fn promote_completed_tags(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let value = self.input.read(cx).value().to_string();
        let mut body = value.clone();
        let mut promoted = Vec::new();

        for token in value.split_inclusive(char::is_whitespace) {
            let trimmed = token.trim_end_matches(char::is_whitespace);
            let whitespace = &token[trimmed.len()..];
            let parsed = parse_meta_tags(trimmed);
            if trimmed.starts_with('#') && parsed.len() == 1 {
                let meta = parsed.into_iter().next().unwrap();
                if !self.tags.iter().any(|tag| tag.label == meta.label) {
                    promoted.push(meta);
                }
                body = body.replacen(token, whitespace, 1);
            }
        }

        if promoted.is_empty() {
            return;
        }
        self.tags.extend(promoted);
        self.input
            .update(cx, |input, cx| input.set_value(body, window, cx));
        cx.notify();
    }

    fn on_key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if event.keystroke.key == "backspace"
            && self.input.read(cx).value().is_empty()
            && self.tags.pop().is_some()
        {
            window.prevent_default();
            cx.stop_propagation();
            cx.notify();
        }
    }

    fn render_tags(&self, cx: &mut Context<Self>) -> impl IntoElement {
        h_flex().gap_1().flex_wrap().children(self.tags.iter().map(|tag| {
            let text = match &tag.value {
                Some(value) => format!("#{}={value}", tag.label),
                None => format!("#{}", tag.label),
            };
            div()
                .px_2()
                .py_0p5()
                .rounded_md()
                .text_xs()
                .bg(cx.theme().secondary)
                .child(text)
        }))
    }

    /// 保存の表示を残してからタスクトレイに戻す。
    fn hide_after_delay(&mut self, cx: &mut Context<Self>) {
        let app_window = self.app_window.clone();
        cx.spawn(async move |view, cx| {
            cx.background_executor().timer(HIDE_AFTER_SAVE).await;
            app_window.hide();
            _ = view.update(cx, |this, cx| {
                this.context = None;
                this.status = None;
                cx.notify();
            });
        })
        .detach();
    }

    /// 明示操作での取り込み（自動取り込みが無効なときの手段）。
    fn pull_foreground_text(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(source) = self.context.clone() else {
            self.status = Some(Status::Error("直前のアプリが分かりません".into()));
            cx.notify();
            return;
        };

        self.pull_selection(Pull::Request { hwnd: source.hwnd }, window, cx);
    }

    /// 直前にフォアグラウンドだったアプリの選択テキストを取り込む。
    ///
    /// 選択範囲を読む標準的な方法が無いため、対象アプリへ Ctrl+C を送って
    /// クリップボード経由で受け取る（クリップボードの内容は上書きされる）。
    /// 待ち時間の間も入力はできるよう、待つのは非同期タスクに任せる。
    fn pull_selection(&mut self, pull: Pull, window: &mut Window, cx: &mut Context<Self>) {
        let app_window = self.app_window.clone();

        cx.spawn_in(window, async move |view, cx| {
            // 明示操作のときだけ、対象に前面を渡して Ctrl+C を送る。
            // 自動取り込みではホットキー側が送信済みなので待つだけでよい。
            let (before_sequence, took_foreground) = match pull {
                Pull::Awaiting(selection) => (selection.before_sequence, false),
                Pull::Request { hwnd } => {
                    let before = foreground::clipboard_sequence();
                    foreground::send_copy_to(hwnd);
                    (before, true)
                }
            };

            let mut waited = Duration::ZERO;
            while foreground::clipboard_sequence() == before_sequence && waited < CLIPBOARD_WAIT {
                cx.background_executor().timer(CLIPBOARD_POLL).await;
                waited += CLIPBOARD_POLL;
            }

            // 連番が変わらない＝選択が無かった、あるいはコピーできないアプリだった。
            let copied = if foreground::clipboard_sequence() == before_sequence {
                None
            } else {
                cx.update(|_, cx| cx.read_from_clipboard())
                    .ok()
                    .flatten()
                    .and_then(|item| item.text())
            };

            if took_foreground {
                app_window.show();
            }

            _ = view.update_in(cx, |this, window, cx| {
                match copied {
                    Some(text) if !text.trim().is_empty() => {
                        this.input.update(cx, |input, cx| {
                            input.insert(&text, window, cx);
                            input.focus(window, cx);
                        });
                        this.status = None;
                    }
                    // 自動取り込みは利用者が頼んだ操作ではないので、黙って何もしない。
                    _ if !took_foreground => {}
                    _ => {
                        this.status = Some(Status::Error(
                            "直前のアプリからテキストを取得できませんでした".into(),
                        ));
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// Esc・閉じるボタンでは終了せず、タスクトレイに残る。
    fn dismiss(&mut self, cx: &mut Context<Self>) {
        self.status = None;
        self.context = None;
        self.app_window.hide();
        cx.notify();
    }

    fn render_context_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let context = self.context.clone();

        h_flex()
            .gap_2()
            .items_center()
            .text_xs()
            .text_color(cx.theme().muted_foreground)
            .when_some(context, |this, context| {
                let label = if context.window_title.trim().is_empty() {
                    format!("#app={}", context.process_name)
                } else {
                    format!("#app={} · {}", context.process_name, context.window_title)
                };

                this.child(
                    div()
                        .px_2()
                        .py_0p5()
                        .rounded_md()
                        .bg(cx.theme().secondary)
                        .max_w(px(360.))
                        .truncate()
                        .child(label),
                )
                .child(
                    Button::new("pull-foreground-text")
                        .ghost()
                        .xsmall()
                        .label("テキストを取り込む")
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.pull_foreground_text(window, cx)
                        })),
                )
            })
    }

    fn render_status(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let (text, is_error): (SharedString, bool) = match &self.status {
            Some(Status::Saved { title, seq }) => {
                (format!("保存しました（#{seq}）: {title}").into(), false)
            }
            Some(Status::Info(message)) => (message.clone(), false),
            Some(Status::Error(message)) => (message.clone(), true),
            None => (SharedString::default(), false),
        };

        div()
            .text_xs()
            .when(is_error, |this| this.text_color(cx.theme().danger))
            .when(!is_error, |this| {
                this.text_color(cx.theme().muted_foreground)
            })
            .child(text)
    }
}

impl Render for CaptureView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .key_context("Minos")
            // 入力欄が処理しなかった Esc だけがここに届く（補完中は補完が閉じるだけ）。
            .on_action(cx.listener(|this, _: &Escape, _window, cx| this.dismiss(cx)))
            .on_key_down(cx.listener(Self::on_key_down))
            .size_full()
            .p_4()
            .gap_3()
            .bg(cx.theme().background)
            .child(self.render_context_bar(cx))
            .child(Input::new(&self.input).prefix(self.render_tags(cx)))
            .child(
                h_flex()
                    .justify_between()
                    .items_center()
                    .child(self.render_status(cx))
                    .child(
                        Button::new("submit")
                            .primary()
                            .label("送信")
                            .on_click(cx.listener(|this, _, window, cx| this.submit(window, cx))),
                    ),
            )
    }
}
