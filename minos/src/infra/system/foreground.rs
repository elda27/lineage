//! 直前にフォアグラウンドだったアプリケーションの観測。
//!
//! docs/ui.md「minos」1.〜2. に対応する。
//!
//! - アプリ情報（実行ファイル名・ウィンドウタイトル）は自動メタ情報になる
//! - そのアプリで選択中のテキストは Ctrl+C を送って取り込む
//!   （クリップボードは一時使用後に復元し、自動で行うかは設定で切り替える）

use std::path::Path;

use windows::Win32::Foundation::{CloseHandle, HWND};
use windows::Win32::System::Com::IDataObject;
use windows::Win32::System::DataExchange::{CountClipboardFormats, GetClipboardSequenceNumber};
use windows::Win32::System::Ole::{OleFlushClipboard, OleGetClipboard, OleSetClipboard};
use windows::Win32::System::Threading::{
    AttachThreadInput, GetCurrentThreadId, OpenProcess, PROCESS_NAME_WIN32,
    PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, SendInput, VIRTUAL_KEY, VK_C,
    VK_CONTROL, VK_MENU,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetForegroundWindow, GetWindowTextW, GetWindowThreadProcessId, SetForegroundWindow,
};
use windows::core::PWSTR;

/// 直前に開いていたアプリケーション。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForegroundApp {
    /// 後からそのウィンドウへ Ctrl+C を送るために保持する。
    pub hwnd: isize,
    pub process_name: String,
    pub window_title: String,
}

/// `Ctrl+C` の前に退避したクリップボード。
///
/// OLE のデータオブジェクトを保持するため、テキストだけでなく画像、ファイル、
/// Office 等の独自形式や遅延レンダリングも元の提供元が対応する範囲で復元できる。
pub enum ClipboardSnapshot {
    Empty,
    Data(IDataObject),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClipboardRestore {
    Restored,
    SkippedBecauseChanged,
}

/// いまフォアグラウンドにあるウィンドウを観測する。
///
/// 自分自身（minos）がフォアグラウンドのときは `None`。
pub fn capture_foreground() -> Option<ForegroundApp> {
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.is_invalid() {
            return None;
        }

        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        if pid == 0 || pid == std::process::id() {
            return None;
        }

        Some(ForegroundApp {
            hwnd: hwnd.0 as isize,
            process_name: process_name_of(pid).unwrap_or_else(|| format!("pid:{pid}")),
            window_title: window_title_of(hwnd),
        })
    }
}

/// クリップボードの更新を検出するための連番。
pub fn clipboard_sequence() -> u32 {
    unsafe { GetClipboardSequenceNumber() }
}

/// 現在のクリップボードを、あとで同じ内容へ戻せる形で保持する。
pub fn snapshot_clipboard() -> windows::core::Result<ClipboardSnapshot> {
    if unsafe { CountClipboardFormats() } == 0 {
        Ok(ClipboardSnapshot::Empty)
    } else {
        unsafe { OleGetClipboard().map(ClipboardSnapshot::Data) }
    }
}

/// Lineage が読み取ったコピー結果から変化していない場合だけ、退避内容を戻す。
///
/// 途中で利用者や別アプリがコピーした内容を、古い退避内容で上書きしないための判定。
pub fn restore_clipboard(
    snapshot: ClipboardSnapshot,
    copied_sequence: u32,
) -> windows::core::Result<ClipboardRestore> {
    if clipboard_sequence() != copied_sequence {
        return Ok(ClipboardRestore::SkippedBecauseChanged);
    }

    match snapshot {
        ClipboardSnapshot::Data(data) => unsafe {
            OleSetClipboard(&data)?;
            // 復元後に minos が終了しても内容が残るよう、遅延形式を実体化して所有権を外す。
            OleFlushClipboard()?;
        },
        ClipboardSnapshot::Empty => unsafe { OleSetClipboard(None::<&IDataObject>)? },
    }

    Ok(ClipboardRestore::Restored)
}

/// 指定ウィンドウを前面に出してから Ctrl+C を送る。
///
/// すでに minos が前面にあるときに、あとから取り込みを指示された場合に使う。
pub fn send_copy_to(hwnd: isize) {
    let hwnd = HWND(hwnd as *mut _);

    unsafe {
        // 前面に出さないとキー入力が届かない。入力キューを一時的に接続して要求する。
        let current_thread = GetCurrentThreadId();
        let target_thread = GetWindowThreadProcessId(hwnd, None);
        let attached = target_thread != 0
            && target_thread != current_thread
            && AttachThreadInput(current_thread, target_thread, true).as_bool();

        let _ = SetForegroundWindow(hwnd);

        let inputs = [
            // Alt+Space のキーアップ前に処理が届いても Ctrl+Alt+C にしない。
            key_event(VK_MENU, true),
            key_event(VK_CONTROL, false),
            key_event(VK_C, false),
            key_event(VK_C, true),
            key_event(VK_CONTROL, true),
        ];
        SendInput(&inputs, size_of::<INPUT>() as i32);

        if attached {
            let _ = AttachThreadInput(current_thread, target_thread, false);
        }
    }
}

fn key_event(key: VIRTUAL_KEY, up: bool) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: key,
                dwFlags: if up {
                    KEYEVENTF_KEYUP
                } else {
                    Default::default()
                },
                ..Default::default()
            },
        },
    }
}

unsafe fn window_title_of(hwnd: HWND) -> String {
    let mut buffer = [0u16; 512];
    let len = unsafe { GetWindowTextW(hwnd, &mut buffer) };
    if len <= 0 {
        return String::new();
    }
    String::from_utf16_lossy(&buffer[..len as usize])
}

/// PID から実行ファイル名を得る。
pub fn process_name_of_pid(pid: u32) -> Option<String> {
    unsafe { process_name_of(pid) }
}

unsafe fn process_name_of(pid: u32) -> Option<String> {
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;

        let mut buffer = [0u16; 1024];
        let mut len = buffer.len() as u32;
        let query = QueryFullProcessImageNameW(
            handle,
            PROCESS_NAME_WIN32,
            PWSTR(buffer.as_mut_ptr()),
            &mut len,
        );
        let _ = CloseHandle(handle);
        query.ok()?;

        let full_path = String::from_utf16_lossy(&buffer[..len as usize]);
        Path::new(&full_path)
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
    }
}
