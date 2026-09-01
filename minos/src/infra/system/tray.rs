//! グローバルホットキー（Alt+Space）とタスクトレイ。
//!
//! どちらも「メッセージループを回しているスレッド」でしか動かない。
//! gpui のループには相乗りできないため、専用スレッドを1本立てて所有させ、
//! 結果だけをチャネルで gpui 側へ渡す。

use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::{Context, Result};
use async_channel::{Receiver, Sender};
use tray_icon::menu::{CheckMenuItem, Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder, TrayIconEvent};
use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    MOD_ALT, MOD_NOREPEAT, RegisterHotKey, UnregisterHotKey, VK_SPACE,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CREATESTRUCTW, CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GWLP_USERDATA,
    GetMessageW, GetWindowLongPtrW, MSG, PostThreadMessageW, RegisterClassW, SetWindowLongPtrW,
    TranslateMessage, WM_CLOSE, WM_ENDSESSION, WM_HOTKEY, WM_NCCREATE, WM_NCDESTROY,
    WM_QUERYENDSESSION, WM_QUIT, WNDCLASSW, WS_EX_TOOLWINDOW, WS_OVERLAPPED,
};
use windows::core::{PCWSTR, w};

use crate::infra::system::{SelectionCapture, SystemEvent, foreground, tray_promotion};

/// `RegisterHotKey` の識別子。プロセス内で一意ならよい。
const HOTKEY_ID: i32 = 1;

/// セッション終了要求を受け取るためのウィンドウクラス名。
const SESSION_WINDOW_CLASS: PCWSTR = w!("minos-session-listener");

/// OS（シャットダウン/ログオフ）またはインストーラ（Restart Manager）から
/// 終了を要求されている、というフラグ。
///
/// 要求は各トップレベルウィンドウへ WM_QUERYENDSESSION → WM_ENDSESSION → WM_CLOSE の順で届く。
/// 最初の問い合わせでこれを立てておき、あとから gpui のウィンドウへ届く WM_CLOSE を
/// 「閉じるボタン」と区別できるようにする。
static SESSION_ENDING: AtomicBool = AtomicBool::new(false);

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

/// OS またはインストーラから終了を要求されているか。
///
/// 真のときは、閉じるボタンと同じ WM_CLOSE で来てもタスクトレイに残ってはいけない。
/// 残るとインストーラは minos を止められず（Restart Manager がシャットダウン失敗を記録し）、
/// トレイアイコンだけを失ったプロセスが居座ることになる。
pub fn session_ending() -> bool {
    SESSION_ENDING.load(Ordering::Relaxed)
}

/// ホットキーとトレイを持つスレッドを起動する。
///
/// `auto_pull_foreground_text` はトレイのチェック項目の初期状態になる。
pub fn spawn(auto_pull_foreground_text: bool) -> Result<SystemBridge> {
    let (sender, receiver) = async_channel::unbounded();
    let (ready_tx, ready_rx) = std::sync::mpsc::channel::<Result<u32>>();

    std::thread::Builder::new()
        .name("minos-system".into())
        .spawn(
            move || match SystemThread::new(sender, auto_pull_foreground_text) {
                Ok(mut thread) => {
                    let _ = ready_tx.send(Ok(thread.thread_id));
                    thread.run();
                }
                Err(error) => {
                    let _ = ready_tx.send(Err(error));
                }
            },
        )
        .context("OS 連携スレッドを起動できません")?;

    let thread_id = ready_rx.recv().context("OS 連携スレッドが応答しません")??;

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
    /// セッション終了要求の受け口。ドロップするとウィンドウごと消える。
    _session: SessionListener,
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
        // Windows 11 は初めて見るアイコンをオーバーフローに隠すので、初回だけ表に出す。
        tray_promotion::promote_in_background();
        let session =
            SessionListener::new(sender.clone()).context("セッション終了の監視を開始できません")?;

        Ok(Self {
            sender,
            thread_id,
            hotkey_registered,
            auto_pull,
            _tray: tray,
            _session: session,
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
    /// 直前のアプリの観測は、minos のウィンドウを出す**前**でなければならない。
    /// 出したあとでは前面が入れ替わり、対象が分からなくなる。
    fn hotkey_event(&self) -> SystemEvent {
        let context = foreground::capture_foreground();

        // Ctrl+C は、gpui 側で元のクリップボードを退避してから送る。
        let selection = context
            .as_ref()
            .filter(|_| self.auto_pull.is_checked())
            .map(|_| SelectionCapture);

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
                MENU_AUTO_PULL => self.emit(SystemEvent::SetAutoPullForegroundText(
                    self.auto_pull.is_checked(),
                )),
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

/// セッション終了要求を受け取るためだけの、目に見えないウィンドウ。
///
/// 表示しないが**トップレベル**であることが必須。WM_QUERYENDSESSION / WM_ENDSESSION は
/// トップレベルウィンドウにしか送られないため、`HWND_MESSAGE` 配下のメッセージ専用
/// ウィンドウでは受け取れない。
///
/// gpui のウィンドウに相乗りできないのでこれを立てる。gpui は WM_CLOSE しか扱わず
/// （`gpui_windows` の `handle_close_msg`）、閉じるボタンと終了要求を区別できない。
struct SessionListener {
    hwnd: HWND,
}

impl SessionListener {
    fn new(sender: Sender<SystemEvent>) -> Result<Self> {
        unsafe {
            let module = GetModuleHandleW(None).context("モジュールハンドルを取得できません")?;
            let instance = HINSTANCE(module.0);

            let class = WNDCLASSW {
                lpfnWndProc: Some(session_proc),
                lpszClassName: SESSION_WINDOW_CLASS,
                hInstance: instance,
                ..Default::default()
            };
            // プロセス内で1度しか作らないので、登録の戻り値（アトム）は見なくてよい。
            RegisterClassW(&class);

            let hwnd = CreateWindowExW(
                // タスクバーにも Alt+Tab にも出さない。ShowWindow を呼ばないので表示もされない。
                WS_EX_TOOLWINDOW,
                SESSION_WINDOW_CLASS,
                PCWSTR::null(),
                WS_OVERLAPPED,
                0,
                0,
                0,
                0,
                // 親を持たせるとトップレベルでなくなり、終了要求が届かなくなる。
                None,
                None,
                Some(instance),
                Some(Box::into_raw(Box::new(sender)) as *const c_void),
            )
            .context("セッション監視ウィンドウを作成できません")?;

            Ok(Self { hwnd })
        }
    }
}

impl Drop for SessionListener {
    fn drop(&mut self) {
        // 所有スレッドから呼ぶので WM_NCDESTROY まで同期的に走り、Sender も解放される。
        unsafe {
            let _ = DestroyWindow(self.hwnd);
        }
    }
}

/// `SessionListener` のウィンドウプロシージャ。
///
/// Sender は生ポインタとしてウィンドウに預ける（このウィンドウと同じ寿命）。
unsafe extern "system" fn session_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe {
        match message {
            WM_NCCREATE => {
                let create = &*(lparam.0 as *const CREATESTRUCTW);
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, create.lpCreateParams as isize);
                return DefWindowProcW(hwnd, message, wparam, lparam);
            }
            WM_NCDESTROY => {
                let previous = SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
                if previous != 0 {
                    drop(Box::from_raw(previous as *mut Sender<SystemEvent>));
                }
                return DefWindowProcW(hwnd, message, wparam, lparam);
            }
            _ => {}
        }

        let sender = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const Sender<SystemEvent>;
        if sender.is_null() {
            return DefWindowProcW(hwnd, message, wparam, lparam);
        }
        let sender = &*sender;

        match message {
            // 「終了してよいか」には常に許可を返す。実際に終わるのは WM_ENDSESSION。
            // ここで終了を始めると、取り消されたシャットダウンでも落ちてしまう。
            WM_QUERYENDSESSION => {
                SESSION_ENDING.store(true, Ordering::Relaxed);
                log::info!("セッション終了の問い合わせを受けました");
                LRESULT(1)
            }
            WM_ENDSESSION => {
                // wParam が FALSE ならセッション終了は取り消された。
                if wparam.0 == 0 {
                    SESSION_ENDING.store(false, Ordering::Relaxed);
                } else {
                    log::info!("セッション終了に従って minos を終了します");
                    let _ = sender.send_blocking(SystemEvent::Quit);
                }
                LRESULT(0)
            }
            // Restart Manager は WM_ENDSESSION で終わらなかったプロセスへ WM_CLOSE も送る。
            // このウィンドウにユーザは触れないので、閉じる要求は終了要求とみなしてよい。
            WM_CLOSE => {
                SESSION_ENDING.store(true, Ordering::Relaxed);
                log::info!("終了要求（WM_CLOSE）を受けて minos を終了します");
                let _ = sender.send_blocking(SystemEvent::Quit);
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, message, wparam, lparam),
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
