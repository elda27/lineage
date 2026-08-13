//! minos — 瞬時に呼び出せる記録用のクイック入力アプリ。
//!
//! docs/ui.md「minos」に対応する。
//! 起動するとタスクトレイに常駐し、Alt+Space で画面中央に現れる。
//! 入力は SQLite（`db/schema.sql`）に保存され、同時に Lineage の hash-chain へ追記される。
//!
//! レイヤ構成は docs/concept/MINIMAL_ARCHITECTURE.md に従う。
//! 依存方向は presentation/infrastructure → application → domain。
//! このファイルは composition root で、具体的な実装を組み立てて注入する役に徹する。

// リリースビルドではコンソールウィンドウを出さない（常駐アプリのため）。
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod application;
mod domain;
mod infrastructure;
mod presentation;

use std::cell::RefCell;
use std::rc::Rc;

use gpui::prelude::*;
use gpui::{
    App, Entity, Pixels, Size, TitlebarOptions, WindowBounds, WindowHandle, WindowKind,
    WindowOptions, px, size,
};
use gpui_component::Root;

use crate::app::Services;
use crate::infrastructure::sqlite::Database;
use crate::domain::settings::Settings;
use crate::infrastructure::system::{
    ForegroundApp, SelectionCapture, SystemEvent, launcher, single_instance, tray,
    window as system_window,
};
use crate::presentation::capture_view::CaptureView;
use crate::presentation::window_control::AppWindow;

/// 自動起動（ログオン時・インストーラ直後）であることを示す引数。
///
/// インストーラが Run 値とインストール完了時の起動の両方に付ける
/// （fullos/src-tauri/wix/minos.wxs）。
const AUTO_START_FLAG: &str = "--autostart";

/// 入力画面の大きさ。
const WINDOW_SIZE: Size<Pixels> = size(px(640.), px(260.));

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    // 自動起動は「利用者が呼んだ」わけではないので、画面を出さずトレイに常駐するだけにする。
    let auto_start = std::env::args().skip(1).any(|arg| arg == AUTO_START_FLAG);

    // 2つ目のプロセスはトレイアイコンを増やすだけなので、既存のウィンドウを出して終わる。
    let _instance = match single_instance::acquire() {
        single_instance::Instance::First(handle) => handle,
        single_instance::Instance::AlreadyRunning if auto_start => {
            // 自動起動が重なっただけ（例: ログオン起動の直後に更新インストーラが起動した）。
            // 利用者は何も要求していないので、黙って譲る。
            log::info!("minos はすでに起動しています");
            return;
        }
        single_instance::Instance::AlreadyRunning => {
            log::info!("minos はすでに起動しています。既存のウィンドウを表示します");
            system_window::activate_other_instance();
            return;
        }
    };

    let database = match Database::open_default() {
        Ok(database) => database,
        Err(error) => {
            log::error!("データベースを開けません: {error:#}");
            std::process::exit(1);
        }
    };

    let services = Services::new(database);
    let settings = services.load_settings().unwrap_or_else(|error| {
        log::warn!("設定を読み込めません（既定値で続行します）: {error:#}");
        Settings::default()
    });

    // ホットキーとトレイは専用スレッドが所有する。ここではその窓口だけを受け取る。
    let bridge = match tray::spawn(settings.auto_pull_foreground_text) {
        Ok(bridge) => bridge,
        Err(error) => {
            log::error!("タスクトレイを初期化できません: {error:#}");
            std::process::exit(1);
        }
    };

    gpui_platform::application().run(move |cx: &mut App| {
        gpui_component::init(cx);

        let app_window = Rc::new(AppWindow::new());
        let events = bridge.events();

        cx.spawn(async move |cx| {
            let view_slot: Rc<RefCell<Option<Entity<CaptureView>>>> = Rc::new(RefCell::new(None));

            let window: WindowHandle<Root> = cx
                .open_window(window_options(auto_start), {
                    let services = services.clone();
                    let app_window = app_window.clone();
                    let view_slot = view_slot.clone();

                    move |window, cx| {
                        let view = cx.new(|cx| {
                            CaptureView::new(services, app_window.clone(), window, cx)
                        });
                        *view_slot.borrow_mut() = Some(view.clone());

                        // 閉じるボタンで終了せず、タスクトレイに残る（docs/ui.md）。
                        //
                        // ただし OS のシャットダウンやインストーラ（Restart Manager）からの
                        // 終了要求も同じ WM_CLOSE で届く。これを拒むと minos は止まらず、
                        // トレイの隠しウィンドウだけが破棄されて「トレイに居ないプロセス」が残る。
                        window.on_window_should_close(cx, move |_window, cx| {
                            if tray::session_ending() {
                                cx.quit();
                                return true;
                            }
                            app_window.hide();
                            false
                        });

                        cx.new(|cx| Root::new(view, window, cx))
                    }
                })
                .expect("ウィンドウを開けませんでした");

            let view = view_slot
                .borrow()
                .clone()
                .expect("入力画面が作成されていません");

            // 最初の表示位置だけは gpui のウィンドウ生成後に整える。
            if auto_start {
                // 隠したまま大きさだけ合わせておく。
                //
                // gpui が WindowOptions の bounds を実際のウィンドウへ適用するのは
                // show=true のときだけで、show=false だと CreateWindowEx の既定
                // （CW_USEDEFAULT）のままになる。ここを飛ばすと、最初の Alt+Space が
                // 画面いっぱいに近い大きさで出る。
                _ = window.update(cx, |_root, window, _cx| window.resize(WINDOW_SIZE));
            } else {
                app_window.show();
            }

            while let Ok(event) = events.recv().await {
                log::debug!("system event: {event:?}");
                match event {
                    SystemEvent::ToggleCapture { context, selection } => {
                        let was_ours = is_foreground_ours();
                        app_window.toggle(was_ours);
                        if app_window.is_visible() {
                            focus_capture(&window, &view, context, selection, cx);
                        }
                    }
                    SystemEvent::ShowCapture { context } => {
                        app_window.show();
                        focus_capture(&window, &view, context, None, cx);
                    }
                    SystemEvent::SetAutoPullForegroundText(enabled) => {
                        let updated = Settings {
                            auto_pull_foreground_text: enabled,
                        };
                        if let Err(error) = services.save_settings(updated) {
                            log::warn!("設定を保存できません: {error:#}");
                        }
                    }
                    SystemEvent::LaunchFullos => match launcher::launch_fullos() {
                        Ok(path) => log::info!("fullos を起動しました: {}", path.display()),
                        Err(error) => log::warn!("fullos を起動できません: {error:#}"),
                    },
                    SystemEvent::VerifyLineage => {
                        app_window.show();
                        _ = window.update(cx, |_root, _window, cx| {
                            view.update(cx, |view, cx| view.show_verification(cx));
                        });
                    }
                    SystemEvent::Quit => {
                        bridge.shutdown();
                        _ = cx.update(|cx| cx.quit());
                        break;
                    }
                }
            }
        })
        .detach();
    });
}

fn window_options(auto_start: bool) -> WindowOptions {
    WindowOptions {
        // 自動起動のときは一度も画面に出さない。
        //
        // 生成してから隠すと、gpui が最初の描画を終えるまでの数十〜数百 ms、
        // 画面の左上にウィンドウが見えてしまう（ログオン直後ほど長い）。
        // show=false なら gpui は CreateWindowEx するだけで表示しないので、
        // 一瞬も出ない。表示は最初の Alt+Space で AppWindow が行う。
        show: !auto_start,
        // 画面中央に出す。実際の再配置は表示のたびに AppWindow が行う。
        window_bounds: Some(WindowBounds::Windowed(gpui::Bounds {
            origin: gpui::point(px(0.), px(0.)),
            size: WINDOW_SIZE,
        })),
        titlebar: Some(TitlebarOptions {
            title: Some(system_window::WINDOW_TITLE.into()),
            ..Default::default()
        }),
        kind: WindowKind::Normal,
        is_resizable: true,
        is_minimizable: false,
        window_min_size: Some(size(px(420.), px(200.))),
        ..Default::default()
    }
}

/// 入力欄にフォーカスを戻し、観測した直前アプリを画面へ渡す。
///
/// ここで gpui の `activate_window()` は呼ばない。前面に出すのは呼び出し元の
/// `AppWindow::show()`（force_foreground）が済ませており、加えて自動起動時は
/// `activate_window()` が保留中の初期配置（show=false のぶん）を吐き出してしまい、
/// 中央に出した直後にウィンドウが左上へ飛ぶ。
fn focus_capture(
    window: &WindowHandle<Root>,
    view: &Entity<CaptureView>,
    context: Option<ForegroundApp>,
    selection: Option<SelectionCapture>,
    cx: &mut gpui::AsyncApp,
) {
    _ = window.update(cx, |_root, window, cx| {
        view.update(cx, |view, cx| {
            view.set_context(context, selection, window, cx)
        });
    });
}

/// いまフォアグラウンドにあるのが minos 自身かどうか。
///
/// 表示中でも他のアプリが前面にいるなら、Alt+Space は「隠す」ではなく「前に出す」であってほしい。
fn is_foreground_ours() -> bool {
    crate::infrastructure::system::foreground::capture_foreground().is_none()
}
