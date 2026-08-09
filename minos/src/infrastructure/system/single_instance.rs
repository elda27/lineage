//! 多重起動の抑止。
//!
//! 2つ目のプロセスが立つとタスクトレイのアイコンが2つ並び、
//! Alt+Space はどちらか一方でしか登録できない（後から起動した方が失敗する）。
//! そのため名前付き Mutex で最初の1つだけを通す。

use windows::Win32::Foundation::{ERROR_ALREADY_EXISTS, GetLastError, HANDLE};
use windows::Win32::System::Threading::CreateMutexW;
use windows::core::w;

/// セッション内で一意な名前。`Local\` なのでユーザセッションごとに1つ。
const MUTEX_NAME: windows::core::PCWSTR = w!("Local\\minos-single-instance");

/// 起動できたかどうか。
pub enum Instance {
    /// このプロセスが最初の1つ。`HANDLE` はプロセス終了まで保持する。
    First(#[allow(dead_code)] HANDLE),
    /// すでに別のプロセスが動いている。
    AlreadyRunning,
}

pub fn acquire() -> Instance {
    unsafe {
        match CreateMutexW(None, true, MUTEX_NAME) {
            // Mutex 自体は作成できても、既存だった場合は ERROR_ALREADY_EXISTS が立つ。
            Ok(handle) if GetLastError() != ERROR_ALREADY_EXISTS => Instance::First(handle),
            Ok(_) => Instance::AlreadyRunning,
            Err(error) => {
                // 判定できないなら起動を止めない（常駐できない方が困る）。
                log::warn!("多重起動の判定に失敗しました: {error}");
                Instance::First(HANDLE::default())
            }
        }
    }
}
