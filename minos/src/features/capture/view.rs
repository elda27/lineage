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

use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

use gpui::prelude::*;
use gpui::{
    App, Context, Entity, Focusable, KeyBinding, MouseButton, PathPromptOptions, SharedString,
    Subscription, Window, actions, div, px,
};
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::input::{Backspace, Escape, Input, InputEvent, InputState};
use gpui_component::{ActiveTheme, Sizable, StyledExt, h_flex, v_flex};

use lineage_core::domain::automation::MemoSnapshot;
use lineage_core::domain::capture::CaptureContext;
use lineage_core::domain::meta::{MetaAssignment, MetaSource, split_completed_tags};

use crate::app::{Services, is_supported_image};
use crate::features::capture::meta_completion::{MetaCompletionProvider, selected_memo_id};
use crate::features::window::AppWindow;
use crate::infra::system::foreground;
use crate::infra::system::{ForegroundApp, SelectionCapture, launcher};

/// 直前アプリへ Ctrl+C を送ってからクリップボードの更新を待つ上限。
const CLIPBOARD_WAIT: Duration = Duration::from_millis(400);
const CLIPBOARD_POLL: Duration = Duration::from_millis(20);

actions!(minos, [InsertHash, InsertPlus, LaunchFullos]);

const KEY_CONTEXT: &str = "Minos";

/// minos の入力画面で使うキーボードショートカットを登録する。
pub fn init(cx: &mut App) {
    cx.bind_keys([
        // 記号を文字として受け取れないキーボード配列でも、補完の起点を入力できるようにする。
        KeyBinding::new("#", InsertHash, Some(KEY_CONTEXT)),
        KeyBinding::new("+", InsertPlus, Some(KEY_CONTEXT)),
        KeyBinding::new("alt-f", LaunchFullos, Some(KEY_CONTEXT)),
    ]);
}

/// 選択テキストの取り込み方。
#[derive(Debug, Clone, Copy)]
enum Pull {
    Automatic { hwnd: isize },
    Explicit { hwnd: isize },
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
    /// 次の送信時にメモへ添付する画像（保存前は元ファイルのパス）。
    images: Vec<PathBuf>,
    /// 選択中なら、次の送信はこのノートを更新する。
    editing_document_id: Option<String>,
    /// 直前にフォアグラウンドだったアプリ（Alt+Space を押した瞬間に観測したもの）。
    context: Option<ForegroundApp>,
    status: Option<Status>,
    saving: bool,
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
                // Ctrl+Enter（gpui-component では secondary-enter）で送信する。
                // Shift も押されていたら、保存後もウィンドウを残して次の入力を受け付ける。
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
            images: Vec::new(),
            editing_document_id: None,
            context: None,
            status: None,
            saving: false,
            _subscriptions: subscriptions,
        }
    }

    /// Alt+Space が押された瞬間に観測した「直前のアプリ」と、
    /// 選択テキストを自動取得する要求を受け取る。
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

        if selection.is_some()
            && let Some(source) = &self.context
        {
            self.pull_selection(Pull::Automatic { hwnd: source.hwnd }, window, cx);
        }
    }

    /// 直前のアプリから作った自動メタ情報を、入力欄のバッジの先頭に並べ直す。
    ///
    /// 前回の呼び出しぶんは入れ替える。同じラベルをユーザが自分で書いていたら、そちらを残す。
    fn refresh_auto_tags(&mut self) {
        // Application/window context is metadata, not an editable or completable tag.
        self.tags.retain(|tag| tag.source != MetaSource::Auto);
    }

    /// 文脈を手放す。バッジも一緒に消して、記録の内容と表示がずれないようにする。
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

    /// 検証結果を画面に出す（トレイメニューから呼ばれる）。
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

    /// fullos を起動する。
    ///
    /// 起動できれば fullos が前面に出るので、minos はタスクトレイに戻る。
    /// 入力中の本文とバッジはそのまま残し、次の Alt+Space で続きを書けるようにする。
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

    /// 入力を確定して保存する。
    fn submit(&mut self, keep_open: bool, window: &mut Window, cx: &mut Context<Self>) {
        if self.saving {
            self.status = Some(Status::Info("前のノートを保存しています".into()));
            cx.notify();
            return;
        }
        let body = self.input.read(cx).value().to_string();
        if body.trim().is_empty() {
            self.status = Some(Status::Error("本文が空です".into()));
            cx.notify();
            return;
        }

        // Context is always persisted as metadata. CaptureMemo promotes it only when
        // the user explicitly entered `#app`.
        let tags = self.tags.clone();
        let images = self.images.clone();
        let document_id = self.editing_document_id.clone();
        let original_context = self.context.clone();
        let result = Services::capture_in_background(
            body.clone(),
            tags.clone(),
            self.capture_context(),
            document_id.clone(),
            images.clone(),
        );

        // DB の完了を待たず、利用者には送信が済んだ状態を先に見せる。
        self.input
            .update(cx, |input, cx| input.set_value("", window, cx));
        self.tags.clear();
        self.images.clear();
        self.editing_document_id = None;
        self.saving = true;
        self.status = Some(Status::Info("保存しています".into()));
        if keep_open {
            self.refresh_auto_tags();
            self.input.update(cx, |input, cx| input.focus(window, cx));
        } else {
            self.app_window.hide();
        }
        cx.notify();

        let app_window = self.app_window.clone();
        cx.spawn_in(window, async move |view, cx| {
            let result = result.recv().await;
            _ = view.update_in(cx, |this, window, cx| {
                this.saving = false;
                match result {
                    Ok(Ok(output)) => {
                        this.status = Some(Status::Saved {
                            title: output.title.into(),
                            seq: output.seq,
                        });
                        if !keep_open {
                            this.clear_context();
                            this.status = None;
                        }
                    }
                    Ok(Err(error)) => {
                        // 楽観的に消した編集内容を戻してから、再入力画面で失敗を伝える。
                        this.tags = tags;
                        this.images = images;
                        this.editing_document_id = document_id;
                        this.context = original_context;
                        this.input.update(cx, |input, cx| {
                            input.set_value(body, window, cx);
                            input.focus(window, cx);
                        });
                        this.status =
                            Some(Status::Error(format!("保存できません: {error}").into()));
                        app_window.show();
                    }
                    Err(error) => {
                        this.tags = tags;
                        this.images = images;
                        this.editing_document_id = document_id;
                        this.context = original_context;
                        this.input.update(cx, |input, cx| {
                            input.set_value(body, window, cx);
                            input.focus(window, cx);
                        });
                        this.status = Some(Status::Error(
                            format!("保存結果を受け取れません: {error}").into(),
                        ));
                        app_window.show();
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn choose_images(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let receiver = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: true,
            prompt: Some("添付する画像を選択".into()),
        });

        cx.spawn_in(window, async move |view, cx| {
            let Ok(Ok(Some(paths))) = receiver.await else {
                return;
            };
            _ = view.update_in(cx, |this, _window, cx| {
                let mut rejected = 0;
                for path in paths {
                    if !is_supported_image(&path) {
                        rejected += 1;
                    } else if !this.images.contains(&path) {
                        this.images.push(path);
                    }
                }
                if rejected > 0 {
                    this.status = Some(Status::Error(
                        "PNG、JPEG、GIF、WebP、BMP 形式の画像を選択してください".into(),
                    ));
                } else {
                    this.status = None;
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// 空白で確定した `#タグ` を本文から取り除き、バッジへ移す。
    ///
    /// 打ち終える前のタグは本文に残す（[`split_completed_tags`] 参照）。
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
                // 同じラベルの自動メタ情報は、利用者が書いた値で置き換える。
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

    /// 本文が空のときの Backspace で、バッジを末尾から1つ外す。
    ///
    /// Backspace は入力欄が action として受け取り、そこで伝播が止まる。
    /// バッジまで届かせるには、入力欄より先に走る capture フェーズで受ける必要がある。
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

    fn insert_completion_prefix(
        &mut self,
        prefix: &'static str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.input.focus_handle(cx).is_focused(window) {
            return;
        }

        self.input
            .update(cx, |input, cx| input.insert(prefix, window, cx));
        cx.stop_propagation();
    }

    /// 補完候補が表示されている間の Esc は、ウィンドウではなく候補だけを閉じる。
    ///
    /// capture フェーズで判定することで、入力コンポーネントの action routing の状態に
    /// 依存せず、Esc を確実に補完 UI が先に消費する。
    fn on_escape(&mut self, _: &Escape, _: &mut Window, cx: &mut Context<Self>) {
        if self.input.read(cx).completion_menu_state().open {
            self.input
                .update(cx, |input, cx| input.dismiss_completion_overlay(cx));
            cx.stop_propagation();
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
                    // 自動付与は利用者が書いたものではないので、色を落として区別する。
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

    fn render_images(&self, cx: &mut Context<Self>) -> impl IntoElement {
        h_flex()
            .gap_1()
            .flex_wrap()
            .children(self.images.iter().enumerate().map(|(index, path)| {
                let name = path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("image");
                h_flex()
                    .id(("remove-image", index))
                    .px_2()
                    .py_0p5()
                    .gap_1()
                    .rounded_md()
                    .text_xs()
                    .bg(cx.theme().secondary)
                    .child(
                        div()
                            .max_w(px(180.))
                            .truncate()
                            .child(format!("画像: {name}")),
                    )
                    .child(
                        div()
                            .id(("remove-image-button", index))
                            .cursor_pointer()
                            .text_color(cx.theme().muted_foreground)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.images.remove(index);
                                cx.notify();
                            }))
                            .child("×"),
                    )
            }))
    }

    /// 入力欄。確定済みのメタ情報は、本文と左端を揃えて同じ枠の中に並べる。
    fn render_input_box(&self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let focused = self.input.focus_handle(cx).is_focused(window);

        v_flex()
            .gap_1()
            // Input が既定（Size::Medium）で使う内側余白と同じにして、バッジと本文の左端を揃える。
            .px(px(12.))
            .py(px(8.))
            .rounded(cx.theme().radius)
            .border_1()
            .border_color(cx.theme().input)
            .bg(cx.theme().input_background())
            .when(focused, |this| this.focused_border(cx))
            // 枠の余白やバッジを押しても、1つの入力欄として本文に入れるようにする。
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, window, cx| {
                    this.input.update(cx, |input, cx| input.focus(window, cx));
                }),
            )
            .when(!self.tags.is_empty(), |this| {
                this.child(self.render_tags(cx))
            })
            // 枠は上の v_flex が描くので、入力欄そのものは枠も余白も持たない。
            .child(Input::new(&self.input).appearance(false).px_0().py_0())
    }

    /// 明示操作での取り込み（自動取り込みが無効なときの手段）。
    fn pull_foreground_text(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(source) = self.context.clone() else {
            self.status = Some(Status::Error("直前のアプリが分かりません".into()));
            cx.notify();
            return;
        };

        self.pull_selection(Pull::Explicit { hwnd: source.hwnd }, window, cx);
    }

    /// 直前にフォアグラウンドだったアプリの選択テキストを取り込む。
    ///
    /// 選択範囲を読む標準的な方法が無いため、対象アプリへ Ctrl+C を送って
    /// クリップボード経由で受け取り、途中で別の更新がなければ元の内容へ戻す。
    /// 待ち時間の間も入力はできるよう、待つのは非同期タスクに任せる。
    fn pull_selection(&mut self, pull: Pull, window: &mut Window, cx: &mut Context<Self>) {
        let app_window = self.app_window.clone();

        cx.spawn_in(window, async move |view, cx| {
            let (hwnd, report_error) = match pull {
                Pull::Automatic { hwnd } => (hwnd, false),
                Pull::Explicit { hwnd } => (hwnd, true),
            };

            // 退避できなくてもテキスト取得は続ける。その場合だけ復元を省略する。
            let original = foreground::snapshot_clipboard().ok();

            let before_sequence = foreground::clipboard_sequence();
            foreground::send_copy_to(hwnd);

            let mut waited = Duration::ZERO;
            while foreground::clipboard_sequence() == before_sequence && waited < CLIPBOARD_WAIT {
                cx.background_executor().timer(CLIPBOARD_POLL).await;
                waited += CLIPBOARD_POLL;
            }

            // 連番が変わらない＝選択が無かった、あるいはコピーできないアプリだった。
            let copied_sequence = foreground::clipboard_sequence();
            let copied = if copied_sequence == before_sequence {
                None
            } else {
                cx.update(|_, cx| cx.read_from_clipboard())
                    .ok()
                    .flatten()
                    .and_then(|item| item.text())
            };

            if let Some(original) = original {
                // 読み取り後の連番が保たれている場合だけ戻す。別操作によるコピーを優先する。
                match foreground::restore_clipboard(original, copied_sequence) {
                    Ok(foreground::ClipboardRestore::Restored) => {}
                    Ok(foreground::ClipboardRestore::SkippedBecauseChanged) => {
                        log::info!(
                            "取り込み中にクリップボードが更新されたため、元の内容を復元しませんでした"
                        );
                    }
                    Err(error) => log::warn!("クリップボードを復元できません: {error}"),
                }
            }

            app_window.show();

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
                    _ if !report_error => {}
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
            .on_action(cx.listener(|this, _: &InsertHash, window, cx| {
                this.insert_completion_prefix("#", window, cx)
            }))
            .on_action(cx.listener(|this, _: &InsertPlus, window, cx| {
                this.insert_completion_prefix("+", window, cx)
            }))
            .on_action(cx.listener(|this, _: &LaunchFullos, _window, cx| this.launch_fullos(cx)))
            // 補完中の Esc は capture フェーズで止まり、それ以外だけがここに届く。
            .on_action(cx.listener(|this, _: &Escape, _window, cx| this.dismiss(cx)))
            .capture_action(cx.listener(Self::on_escape))
            .capture_action(cx.listener(Self::on_backspace))
            .size_full()
            .p_4()
            .gap_3()
            .bg(cx.theme().background)
            .child(self.render_input_box(window, cx))
            .when(!self.images.is_empty(), |this| {
                this.child(self.render_images(cx))
            })
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
                            .child(
                                Button::new("attach-images")
                                    .ghost()
                                    .xsmall()
                                    .label("画像を添付")
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.choose_images(window, cx)
                                    })),
                            )
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
