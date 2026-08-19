//! minos の入力画面。
//!
//! docs/ui.md「minos」1.〜3. に対応する。
//!
//! - テキストボックス1つと送信ボタン1つ
//! - Ctrl+Enter で送信、Ctrl+Shift+Enter でウィンドウを残して連続送信
//! - `#` でメタ情報を補完（候補は過去の入力から学習したもの）
//! - 確定したメタ情報は、自動付与ぶんも含めて入力欄の中にバッジとして並ぶ。
//!   バッジの ×、または本文が空のときの Backspace で外せる
//! - 直前のアプリ情報は自動メタ情報として付き、そのアプリの選択テキストも取り込める
//!   （自動で取り込むかはトレイメニューの設定で切り替える）
//! - fullos をここから起動できる（トレイメニューの「fullos を起動」と同じ）

use std::rc::Rc;
use std::time::Duration;

use gpui::prelude::*;
use gpui::{
    App, Context, Entity, Focusable, KeyBinding, MouseButton, SharedString, Subscription, Window,
    actions, div, px,
};
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::input::{Backspace, Escape, Input, InputEvent, InputState};
use gpui_component::{ActiveTheme, Sizable, StyledExt, h_flex, v_flex};

use lineage_core::domain::automation::MemoSnapshot;
use lineage_core::domain::capture::CaptureContext;
use lineage_core::domain::meta::{MetaAssignment, MetaSource, split_completed_tags};

use crate::app::Services;
use crate::features::capture::meta_completion::{MetaCompletionProvider, selected_memo_id};
use crate::features::window::AppWindow;
use crate::infra::system::foreground;
use crate::infra::system::{ForegroundApp, SelectionCapture, launcher};

/// 保存の手応えを見せてから引っ込むまでの時間。
const HIDE_AFTER_SAVE: Duration = Duration::from_millis(700);
/// 直前アプリへ Ctrl+C を送ってからクリップボードの更新を待つ上限。
const CLIPBOARD_WAIT: Duration = Duration::from_millis(400);
const CLIPBOARD_POLL: Duration = Duration::from_millis(20);

actions!(minos, [LaunchFullos]);

const KEY_CONTEXT: &str = "Minos";

/// minos の入力画面で使うキーボードショートカットを登録する。
pub fn init(cx: &mut App) {
    cx.bind_keys([KeyBinding::new("alt-f", LaunchFullos, Some(KEY_CONTEXT))]);
}

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
    /// 入力欄内にバッジとして表示する確定済みのメタ情報。
    ///
    /// 自動付与（直前のアプリ）とユーザ入力の両方が並ぶ。ここから消えたものは記録にも残らない。
    tags: Vec<MetaAssignment>,
    /// 選択中なら、次の送信はこのノートを更新する。
    editing_document_id: Option<String>,
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
                .placeholder("いま気づいたことを書く（Ctrl+Enter で送信、Shift 追加で連続入力）");
            state.lsp.completion_provider =
                Some(Rc::new(MetaCompletionProvider::new(services.clone())));
            state
        });

        let subscriptions = vec![cx.subscribe_in(&input, window, {
            move |this, _, event: &InputEvent, window, cx| {
                match event {
                    InputEvent::PressEnter {
                        secondary: true,
                        shift,
                    } => this.submit(*shift, window, cx),
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
            editing_document_id: None,
            context: None,
            status: None,
            _subscriptions: subscriptions,
        }
    }

    pub fn set_context(
        &mut self,
        context: Option<ForegroundApp>,
        selection: Option<SelectionCapture>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(app) = context {
            self.context = Some(app);
        }
        self.input.update(cx, |input, cx| input.focus(window, cx));
        cx.notify();

        if let Some(selection) = selection {
            self.pull_selection(Pull::Awaiting(selection), window, cx);
        }
    }

    fn refresh_auto_tags(&mut self) {
        self.tags.retain(|tag| tag.source != MetaSource::Auto);
    }

    fn clear_context(&mut self) {
        self.context = None;
        self.refresh_auto_tags();
    }

    fn capture_context(&self) -> Option<CaptureContext> {
        self.context.as_ref().map(|app| CaptureContext {
            process_name: app.process_name.clone(),
            window_title: app.window_title.clone(),
        })
    }

    pub fn show_verification(&mut self, cx: &mut Context<Self>) {
        use lineage_core::domain::lineage::VerifyResult;

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

    fn launch_fullos(&mut self, cx: &mut Context<Self>) {
        match launcher::launch_fullos() {
            Ok(path) => {
                log::info!("fullos を起動しました: {}", path.display());
                self.status = None;
                self.app_window.hide();
            }
            Err(error) => {
                log::warn!("fullos を起動できません: {error:#}");
                self.status = Some(Status::Error(
                    format!("fullos を起動できません: {error}").into(),
                ));
            }
        }
        cx.notify();
    }

    fn submit(&mut self, keep_open: bool, window: &mut Window, cx: &mut Context<Self>) {
        let body = self.input.read(cx).value().to_string();
        if body.trim().is_empty() {
            self.status = Some(Status::Error("本文が空です".into()));
            cx.notify();
            return;
        }

        let capture_context = self.capture_context();

        match self.services.capture(
            body,
            self.tags.clone(),
            capture_context,
            self.editing_document_id.clone(),
        ) {
            Ok(output) => {
                self.input
                    .update(cx, |input, cx| input.set_value("", window, cx));
                self.tags.clear();
                self.editing_document_id = None;
                self.status = Some(Status::Saved {
                    title: output.title.into(),
                    seq: output.seq,
                });
                if keep_open {
                    self.refresh_auto_tags();
                    self.input.update(cx, |input, cx| input.focus(window, cx));
                }
                cx.notify();
                if !keep_open {
                    self.hide_after_delay(cx);
                }
            }
            Err(error) => {
                self.status = Some(Status::Error(format!("保存できません: {error}").into()));
                cx.notify();
            }
        }
    }

    fn promote_completed_tags(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let value = self.input.read(cx).value().to_string();
        if let Some(document_id) = selected_memo_id(&value) {
            match self.services.memo(document_id) {
                Ok(Some(memo)) => self.restore_memo(memo, window, cx),
                Ok(None) => {
                    self.input
                        .update(cx, |input, cx| input.set_value("", window, cx));
                    self.status = Some(Status::Error("選択したノートが見つかりません".into()));
                    cx.notify();
                }
                Err(error) => {
                    self.input
                        .update(cx, |input, cx| input.set_value("", window, cx));
                    self.status = Some(Status::Error(
                        format!("過去のノートを取得できません: {error}").into(),
                    ));
                    cx.notify();
                }
            }
            return;
        }
        let (body, promoted) = split_completed_tags(&value);
        if promoted.is_empty() {
            return;
        }

        for meta in promoted {
            match self.tags.iter().position(|tag| tag.label == meta.label) {
                Some(index) if self.tags[index].source == MetaSource::Auto => {
                    self.tags[index] = meta
                }
                Some(_) => {}
                None => self.tags.push(meta),
            }
        }

        self.input
            .update(cx, |input, cx| input.set_value(body, window, cx));
        cx.notify();
    }

    fn restore_memo(&mut self, memo: MemoSnapshot, window: &mut Window, cx: &mut Context<Self>) {
        self.editing_document_id = Some(memo.id);
        self.tags = memo.metas;
        self.input.update(cx, |input, cx| {
            input.set_value(memo.body_text, window, cx);
            input.focus(window, cx);
        });
        self.status = Some(Status::Info(
            "過去のノートを復元しました。続けて入力できます".into(),
        ));
        cx.notify();
    }

    fn on_backspace(&mut self, _: &Backspace, window: &mut Window, cx: &mut Context<Self>) {
        if !self.input.read(cx).value().is_empty()
            || !self.input.focus_handle(cx).is_focused(window)
        {
            return;
        }

        if self.tags.pop().is_some() {
            cx.stop_propagation();
            cx.notify();
        }
    }

    fn render_tags(&self, cx: &mut Context<Self>) -> impl IntoElement {
        h_flex()
            .gap_1()
            .flex_wrap()
            .children(self.tags.iter().enumerate().map(|(index, tag)| {
                let text = match &tag.value {
                    Some(value) => format!("#{}={value}", tag.label),
                    None => format!("#{}", tag.label),
                };
                let label = tag.label.clone();

                h_flex()
                    .id(("remove-tag", index))
                    .px_2()
                    .py_0p5()
                    .gap_1()
                    .rounded_md()
                    .text_xs()
                    .bg(cx.theme().secondary)
                    .when(tag.source == MetaSource::Auto, |this| {
                        this.text_color(cx.theme().muted_foreground)
                    })
                    .max_w(px(320.))
                    .child(div().min_w_0().truncate().child(text))
                    .child(
                        div()
                            .id(("remove-tag-button", index))
                            .flex_none()
                            .cursor_pointer()
                            .text_color(cx.theme().muted_foreground)
                            .hover(|style| style.text_color(cx.theme().foreground))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.tags.retain(|tag| tag.label != label);
                                cx.notify();
                            }))
                            .child("×"),
                    )
            }))
    }

    fn render_input_box(&self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let focused = self.input.focus_handle(cx).is_focused(window);

        v_flex()
            .gap_1()
            .px(px(12.))
            .py(px(8.))
            .rounded(cx.theme().radius)
            .border_1()
            .border_color(cx.theme().input)
            .bg(cx.theme().input_background())
            .when(focused, |this| this.focused_border(cx))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, window, cx| {
                    this.input.update(cx, |input, cx| input.focus(window, cx));
                }),
            )
            .when(!self.tags.is_empty(), |this| {
                this.child(self.render_tags(cx))
            })
            .child(Input::new(&self.input).appearance(false).px_0().py_0())
    }

    fn hide_after_delay(&mut self, cx: &mut Context<Self>) {
        let app_window = self.app_window.clone();
        cx.spawn(async move |view, cx| {
            cx.background_executor().timer(HIDE_AFTER_SAVE).await;
            app_window.hide();
            _ = view.update(cx, |this, cx| {
                this.clear_context();
                this.status = None;
                cx.notify();
            });
        })
        .detach();
    }

    fn pull_foreground_text(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(source) = self.context.clone() else {
            self.status = Some(Status::Error("直前のアプリが分かりません".into()));
            cx.notify();
            return;
        };

        self.pull_selection(Pull::Request { hwnd: source.hwnd }, window, cx);
    }

    fn pull_selection(&mut self, pull: Pull, window: &mut Window, cx: &mut Context<Self>) {
        let app_window = self.app_window.clone();

        cx.spawn_in(window, async move |view, cx| {
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

    fn dismiss(&mut self, cx: &mut Context<Self>) {
        self.status = None;
        self.clear_context();
        self.app_window.hide();
        cx.notify();
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
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .key_context(KEY_CONTEXT)
            .on_action(cx.listener(|this, _: &LaunchFullos, _window, cx| this.launch_fullos(cx)))
            .on_action(cx.listener(|this, _: &Escape, _window, cx| this.dismiss(cx)))
            .capture_action(cx.listener(Self::on_backspace))
            .size_full()
            .p_4()
            .gap_3()
            .bg(cx.theme().background)
            .child(self.render_input_box(window, cx))
            .child(
                h_flex()
                    .justify_between()
                    .items_center()
                    .gap_2()
                    .child(self.render_status(cx))
                    .child(
                        h_flex()
                            .gap_2()
                            .items_center()
                            .when(self.context.is_some(), |this| {
                                this.child(
                                    Button::new("pull-foreground-text")
                                        .ghost()
                                        .xsmall()
                                        .label("直前のアプリからテキストを取り込む")
                                        .on_click(cx.listener(|this, _, window, cx| {
                                            this.pull_foreground_text(window, cx)
                                        })),
                                )
                            })
                            .child(
                                Button::new("launch-fullos")
                                    .ghost()
                                    .xsmall()
                                    .label("fullos を開く (Alt+F)")
                                    .on_click(
                                        cx.listener(|this, _, _window, cx| this.launch_fullos(cx)),
                                    ),
                            )
                            .child(Button::new("submit").primary().label("送信").on_click(
                                cx.listener(|this, _, window, cx| this.submit(false, window, cx)),
                            )),
                    ),
            )
    }
}
