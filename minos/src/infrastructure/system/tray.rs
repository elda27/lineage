//! グローバルホットキー（Alt+Space）とタスクトレイ。
//!
//! どちらも「メッセージループを回しているスレッド」でしか動かない。
//! gpui のループには相乗りできないため、専用スレッドを1本立てて所有させ、
//! 結果だけをチャネルで gpui 側へ渡す。

use anyhow::{Context, Result};
use async_channel::{Receiver, Sender};
use tray_icon::menu::{CheckMenuItem, Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder, TrayIconEvent};
use windows::Win32::Foundation::{LPARAM, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    MOD_ALT, MOD_NOREPEAT, RegisterHotKey, UnregisterHotKey, VK_SPACE,
};
use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, GetMessageW, MSG, PostThreadMessageW, TranslateMessage, WM_HOTKEY, WM_QUIT,
};

use crate::infrastructure::system::{SelectionCapture, SystemEvent, foreground};

/// `RegisterHotKey` の識別子。プロセス内で一意ならよい。
const HOTKEY_ID: i32 = 1;

const MENU_SHOW: &str = "minos.show";
const MENU_AUTO_PULL: &str = "minos.auto-pull";
const MENU_LAUNCH_FULLOS: &str = "minos.launch-fullos";
const MENU_VERIFY: &str = "minos.verify";
const MENU_QUIT: &str = "minos.quit";

/// OS 側スレッドへの窓口。
pub struct SystemBridge {
    events: Receiver<SystemEvent>,
    thread_id: u32,
}

impl SystemBridge {
    /// gpui 側が購読するイベントストリーム。
    pub fn events(&self) -> Receiver<SystemEvent> {
        self.events.clone()
    }

    /// OS 側スレッドを終了させる。
    pub fn shutdown(&self) {
        unsafe {
            let _ = PostThreadMessageW(self.thread_id, WM_QUIT, WPARAM(0), LPARAM(0));
        }
    }
}

/// ホットキーとトレイを持つスレッドを起動する。
///
/// `auto_pull_foreground_text` はトレイのチェック項目の初期状態になる。
pub fn spawn(auto_pull_foreground_text: bool) -> Result<SystemBridge> {
    let (sender, receiver) = async_channel::unbounded();
    let (ready_tx, ready_rx) = std::sync::mpsc::channel::<Result<u32>>();

    std::thread::Builder::new()
        .name("minos-system".into())
        .spawn(move || match SystemThread::new(sender, auto_pull_foreground_text) {
            Ok(mut thread) => {
                let _ = ready_tx.send(Ok(thread.thread_id));
                thread.run();
            }
            Err(error) => {
                let _ = ready_tx.send(Err(error));
            }
        })
        .context("OS 連携スレッドを起動できません")?;

    let thread_id = ready_rx
        .recv()
        .context("OS 連携スレッドが応答しません")??;

    Ok(SystemBridge {
        events: receiver,
        thread_id,
    })
}

struct SystemThread {
    sender: Sender<SystemEvent>,
    thread_id: u32,
    hotkey_registered: bool,
    /// 自動取り込みの ON/OFF。チェック状態がそのまま設定値になる。
    auto_pull: CheckMenuItem,
    /// ドロップするとトレイアイコンが消えるので保持し続ける。
    _tray: TrayIcon,
}

impl SystemThread {
    fn new(sender: Sender<SystemEvent>, auto_pull_foreground_text: bool) -> Result<Self> {
        let thread_id = unsafe { windows::Win32::System::Threading::GetCurrentThreadId() };

        // Alt+Space。MOD_NOREPEAT で押しっぱなしの連射を抑える。
        let hotkey_registered =
            unsafe { RegisterHotKey(None, HOTKEY_ID, MOD_ALT | MOD_NOREPEAT, VK_SPACE.0 as u32) }
                .is_ok();
        if !hotkey_registered {
            log::warn!("Alt+Space を登録できませんでした（他のアプリが使用中の可能性があります）");
        }

        let auto_pull = CheckMenuItem::with_id(
            MenuId::new(MENU_AUTO_PULL),
            "直前アプリのテキストを自動で取り込む",
            true,
            auto_pull_foreground_text,
            None,
        );
        let tray = build_tray(&auto_pull).context("タスクトレイのアイコンを作成できません")?;

        Ok(Self {
            sender,
            thread_id,
            hotkey_registered,
            auto_pull,
            _tray: tray,
        })
    }

    fn run(&mut self) {
        let mut message = MSG::default();

        loop {
            let received = unsafe { GetMessageW(&mut message, None, 0, 0) };
            if received.0 <= 0 {
                break;
            }

            if message.message == WM_HOTKEY && message.wParam.0 as i32 == HOTKEY_ID {
                self.emit(self.hotkey_event());
            }

            unsafe {
                let _ = TranslateMessage(&message);
                DispatchMessageW(&message);
            }

            self.drain_tray_events();
        }

        if self.hotkey_registered {
            unsafe {
                let _ = UnregisterHotKey(None, HOTKEY_ID);
            }
        }
    }

    /// Alt+Space を押した瞬間のイベントを作る。
    ///
    /// 直前のアプリの観測も Ctrl+C の送信も、minos のウィンドウを出す**前**でなければならない。
    /// 出したあとでは前面が入れ替わり、対象が分からなくなる。
    fn hotkey_event(&self) -> SystemEvent {
        let context = foreground::capture_foreground();

        // 自動取り込みが有効なときだけコピーを試みる。
        // クリップボードは上書きされるが、それを承知の設定。
        let selection = context.as_ref().filter(|_| self.auto_pull.is_checked()).map(|_| {
            let before_sequence = foreground::clipboard_sequence();
            foreground::copy_from_foreground();
            SelectionCapture { before_sequence }
        });

        SystemEvent::ToggleCapture { context, selection }
    }

    /// トレイとメニューのイベントは WndProc からチャネルに積まれるので、
    /// メッセージを1つ処理するたびに拾い上げる。
    fn drain_tray_events(&self) {
        while let Ok(event) = TrayIconEvent::receiver().try_recv() {
            if let TrayIconEvent::Click {
                button: tray_icon::MouseButton::Left,
                button_state: tray_icon::MouseButtonState::Up,
                ..
            } = event
            {
                // クリックで呼び出す場合、前面はトレイ操作の前のアプリ。
                // 明示操作ではないので Ctrl+C までは送らない。
                self.emit(SystemEvent::ToggleCapture {
                    context: foreground::capture_foreground(),
                    selection: None,
                });
            }
        }

        while let Ok(event) = MenuEvent::receiver().try_recv() {
            match event.id.as_ref() {
                MENU_SHOW => self.emit(SystemEvent::ShowCapture {
                    context: foreground::capture_foreground(),
                }),
                MENU_AUTO_PULL => {
                    self.emit(SystemEvent::SetAutoPullForegroundText(
                        self.auto_pull.is_checked(),
                    ))
                }
                MENU_LAUNCH_FULLOS => self.emit(SystemEvent::LaunchFullos),
                MENU_VERIFY => self.emit(SystemEvent::VerifyLineage),
                MENU_QUIT => self.emit(SystemEvent::Quit),
                other => log::warn!("未知のメニュー項目: {other}"),
            }
        }
    }

    fn emit(&self, event: SystemEvent) {
        if self.sender.send_blocking(event).is_err() {
            // gpui 側が終了している。次の WM_QUIT でこのスレッドも落ちる。
            unsafe {
                let _ = PostThreadMessageW(self.thread_id, WM_QUIT, WPARAM(0), LPARAM(0));
            }
        }
    }
}

fn build_tray(auto_pull: &CheckMenuItem) -> Result<TrayIcon> {
    let menu = Menu::new();
    menu.append(&MenuItem::with_id(
        MenuId::new(MENU_SHOW),
        "minos を開く (Alt+Space)",
        true,
        None,
    ))?;
    menu.append(&PredefinedMenuItem::separator())?;
    menu.append(auto_pull)?;
    menu.append(&PredefinedMenuItem::separator())?;
    menu.append(&MenuItem::with_id(
        MenuId::new(MENU_LAUNCH_FULLOS),
        "fullos を起動",
        true,
        None,
    ))?;
    menu.append(&MenuItem::with_id(
        MenuId::new(MENU_VERIFY),
        "記録の整合性を検証",
        true,
        None,
    ))?;
    menu.append(&PredefinedMenuItem::separator())?;
    menu.append(&MenuItem::with_id(
        MenuId::new(MENU_QUIT),
        "minos を終了",
        true,
        None,
    ))?;

    let tray = TrayIconBuilder::new()
        .with_tooltip("minos")
        .with_icon(tray_icon()?)
        .with_menu(Box::new(menu))
        // 左クリックはトグル、右クリックがメニュー（docs/ui.md）。
        .with_menu_on_left_click(false)
        .with_menu_on_right_click(true)
        .build()?;

    Ok(tray)
}

/// アイコンはアセットを持たずコードで描く（配布物を exe 1つに保つため）。
/// 角丸の四角に、記録を表す横線を2本入れただけの図形。
fn tray_icon() -> Result<Icon> {
    const SIZE: u32 = 32;
    const RADIUS: i32 = 7;
    let background = [0x2f, 0x6f, 0xed, 0xff];
    let ink = [0xff, 0xff, 0xff, 0xff];

    let mut rgba = vec![0u8; (SIZE * SIZE * 4) as usize];
    for y in 0..SIZE as i32 {
        for x in 0..SIZE as i32 {
            let pixel = (((y * SIZE as i32) + x) * 4) as usize;
            if !inside_rounded_square(x, y, SIZE as i32, RADIUS) {
                continue;
            }

            let on_line = (10..=22).contains(&x) && (matches!(y, 12..=14) || matches!(y, 18..=20));
            rgba[pixel..pixel + 4].copy_from_slice(if on_line { &ink } else { &background });
        }
    }

    Icon::from_rgba(rgba, SIZE, SIZE).context("トレイアイコンの生成に失敗しました")
}

fn inside_rounded_square(x: i32, y: i32, size: i32, radius: i32) -> bool {
    let corner_x = if x < radius {
        Some(radius)
    } else if x >= size - radius {
        Some(size - radius - 1)
    } else {
        None
    };
    let corner_y = if y < radius {
        Some(radius)
    } else if y >= size - radius {
        Some(size - radius - 1)
    } else {
        None
    };

    match (corner_x, corner_y) {
        (Some(cx), Some(cy)) => {
            let (dx, dy) = (x - cx, y - cy);
            dx * dx + dy * dy <= radius * radius
        }
        _ => true,
    }
}
