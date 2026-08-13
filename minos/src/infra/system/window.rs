//! 自分自身のウィンドウの表示制御。
//!
//! gpui はウィンドウの hide/show を公開していないため、HWND を自力で引き当てて Win32 で操作する。
//! 閉じるボタンで終了せずタスクトレイに残る（docs/ui.md「minos」）ためにはこれが必要。

use windows::Win32::Foundation::{HWND, LPARAM, POINT, RECT};
use windows::Win32::Graphics::Gdi::{
    GetMonitorInfoW, HMONITOR, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromPoint,
};
use windows::Win32::System::Threading::{AttachThreadInput, GetCurrentThreadId};
use windows::Win32::UI::Input::KeyboardAndMouse::SetFocus;
use windows::Win32::UI::WindowsAndMessaging::{
    BringWindowToTop, EnumWindows, GetClassNameW, GetCursorPos, GetForegroundWindow, GetWindowRect,
    GetWindowTextW, GetWindowThreadProcessId, HWND_NOTOPMOST, HWND_TOPMOST, IsWindowVisible,
    SW_HIDE, SW_SHOW, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_SHOWWINDOW, SetForegroundWindow,
    SetWindowPos, ShowWindow,
};
use windows::core::BOOL;

use crate::infrastructure::system::foreground;

/// gpui が登録しているウィンドウクラス名（gpui_windows の `WINDOW_CLASS_NAME`）。
const GPUI_WINDOW_CLASS: &str = "Zed::Window";

/// minos のウィンドウタイトル。同クラスのウィンドウが複数あるときの目印にする。
pub const WINDOW_TITLE: &str = "minos";

/// 自プロセスの gpui ウィンドウ（＝ minos の入力画面）の HWND を探す。
///
/// gpui は HWND を公開していないので、クラス名とプロセス ID から引き当てる。
pub fn find_app_window() -> Option<isize> {
    let ours = std::process::id();
    find_window(|_, pid, title| pid == ours && title == WINDOW_TITLE)
        // タイトルがまだ載っていない場合に備えて、自プロセスの gpui ウィンドウなら拾う。
        .or_else(|| find_window(|_, pid, _| pid == ours))
}

pub fn is_visible(hwnd: isize) -> bool {
    unsafe { IsWindowVisible(HWND(hwnd as *mut _)).as_bool() }
}

pub fn hide(hwnd: isize) {
    unsafe {
        let _ = ShowWindow(HWND(hwnd as *mut _), SW_HIDE);
    }
}

/// カーソルのあるモニタの中央に置いて表示する。
///
/// 一度 topmost にしてから notopmost に戻すことで「最前面に出るが、最前面固定はしない」。
pub fn show_centered(hwnd: isize) {
    let hwnd = HWND(hwnd as *mut _);

    unsafe {
        let (x, y) = centered_position(hwnd).unwrap_or((0, 0));
        let _ = SetWindowPos(
            hwnd,
            Some(HWND_TOPMOST),
            x,
            y,
            0,
            0,
            SWP_NOSIZE | SWP_NOACTIVATE | SWP_SHOWWINDOW,
        );
        let _ = ShowWindow(hwnd, SW_SHOW);
        let _ = SetWindowPos(
            hwnd,
            Some(HWND_NOTOPMOST),
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
        );
    }
}

/// 自分のウィンドウを確実に前面に出してフォーカスする。
///
/// Windows は「入力を受け取っていないプロセス」が前面に出るのを拒むため、
/// 現在のフォアグラウンドスレッドに入力キューを一時的に接続してから要求する。
pub fn force_foreground(hwnd: isize) {
    let hwnd = HWND(hwnd as *mut _);

    unsafe {
        let foreground = GetForegroundWindow();
        let foreground_thread = GetWindowThreadProcessId(foreground, None);
        let current_thread = GetCurrentThreadId();
        let attached = foreground_thread != 0
            && foreground_thread != current_thread
            && AttachThreadInput(current_thread, foreground_thread, true).as_bool();

        let _ = BringWindowToTop(hwnd);
        let _ = SetForegroundWindow(hwnd);
        let _ = SetFocus(Some(hwnd));

        if attached {
            let _ = AttachThreadInput(current_thread, foreground_thread, false);
        }
    }
}

/// すでに動いている minos のウィンドウを表示して前面に出す。
///
/// 多重起動したときに「何も起きない」のを避けるための処理。
/// gpui のウィンドウクラス名は Zed と共通なので、
/// 「同じ実行ファイル名のプロセスが持つ、タイトルが minos のウィンドウ」で絞り込む。
pub fn activate_other_instance() {
    let ours = std::process::id();
    let Some(hwnd) = find_window(|_, pid, title| {
        pid != ours && title == WINDOW_TITLE && is_same_executable(pid)
    }) else {
        log::warn!("既存の minos のウィンドウが見つかりませんでした");
        return;
    };

    unsafe {
        let _ = ShowWindow(HWND(hwnd as *mut _), SW_SHOW);
    }
    force_foreground(hwnd);
}

fn is_same_executable(pid: u32) -> bool {
    let ours = std::env::current_exe()
        .ok()
        .and_then(|path| path.file_name().map(|name| name.to_string_lossy().into_owned()));
    match (ours, foreground::process_name_of_pid(pid)) {
        (Some(ours), Some(theirs)) => ours.eq_ignore_ascii_case(&theirs),
        _ => false,
    }
}

struct FindState<'a> {
    accept: &'a dyn Fn(isize, u32, &str) -> bool,
    found: Option<isize>,
}

/// gpui のウィンドウクラスを持つ最初のウィンドウを、述語で絞り込んで探す。
fn find_window(accept: impl Fn(isize, u32, &str) -> bool) -> Option<isize> {
    let mut state = FindState {
        accept: &accept,
        found: None,
    };

    // コールバックが false を返すと EnumWindows は Err を返すが、ここでは探索結果だけを見る。
    let _ = unsafe {
        EnumWindows(
            Some(enum_windows_proc),
            LPARAM(&mut state as *mut FindState as isize),
        )
    };

    state.found
}

unsafe extern "system" fn enum_windows_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
    const CONTINUE: BOOL = BOOL(1);
    const STOP: BOOL = BOOL(0);

    let state = unsafe { &mut *(lparam.0 as *mut FindState) };

    if read_window_text(|buffer| unsafe { GetClassNameW(hwnd, buffer) }) != GPUI_WINDOW_CLASS {
        return CONTINUE;
    }

    let mut pid = 0u32;
    unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)) };
    let title = read_window_text(|buffer| unsafe { GetWindowTextW(hwnd, buffer) });

    if (state.accept)(hwnd.0 as isize, pid, &title) {
        state.found = Some(hwnd.0 as isize);
        return STOP;
    }
    CONTINUE
}

/// 中央配置の左上座標を求める。
unsafe fn centered_position(hwnd: HWND) -> Option<(i32, i32)> {
    let mut cursor = POINT::default();
    unsafe { GetCursorPos(&mut cursor) }.ok()?;

    let monitor: HMONITOR = unsafe { MonitorFromPoint(cursor, MONITOR_DEFAULTTONEAREST) };
    let mut info = MONITORINFO {
        cbSize: size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    if !unsafe { GetMonitorInfoW(monitor, &mut info) }.as_bool() {
        return None;
    }

    let mut window = RECT::default();
    unsafe { GetWindowRect(hwnd, &mut window) }.ok()?;

    let work = info.rcWork;
    let width = window.right - window.left;
    let height = window.bottom - window.top;
    Some((
        work.left + ((work.right - work.left) - width) / 2,
        work.top + ((work.bottom - work.top) - height) / 2,
    ))
}

/// `GetWindowTextW` 系の「バッファに書いて長さを返す」API を Rust の文字列にする。
fn read_window_text(mut fill: impl FnMut(&mut [u16]) -> i32) -> String {
    let mut buffer = [0u16; 512];
    let len = fill(&mut buffer);
    if len <= 0 {
        return String::new();
    }
    String::from_utf16_lossy(&buffer[..len as usize])
}
