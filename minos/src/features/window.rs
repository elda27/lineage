//! 入力画面の表示/非表示。
//!
//! gpui はウィンドウの hide を持たないため、HWND を1度だけ引き当てて保持し、
//! 以降はそれを使って表示状態を切り替える。

use std::cell::Cell;

use crate::infra::system::window;

/// minos の入力ウィンドウ。
#[derive(Default)]
pub struct AppWindow {
    hwnd: Cell<Option<isize>>,
}

impl AppWindow {
    pub fn new() -> Self {
        Self::default()
    }

    /// 中央に出して前面に持ってくる。
    pub fn show(&self) {
        if let Some(hwnd) = self.hwnd() {
            window::show_centered(hwnd);
            window::force_foreground(hwnd);
        }
    }

    /// タスクトレイに引っ込める（終了はしない）。
    pub fn hide(&self) {
        if let Some(hwnd) = self.hwnd() {
            window::hide(hwnd);
        }
    }

    pub fn is_visible(&self) -> bool {
        self.hwnd().is_some_and(window::is_visible)
    }

    /// 表示中なら隠す、隠れているなら出す。
    ///
    /// 表示中でも前面にいない場合は、隠さず前面に出す方が意図に合う。
    pub fn toggle(&self, was_foreground_ours: bool) {
        if self.is_visible() && was_foreground_ours {
            self.hide();
        } else {
            self.show();
        }
    }

    fn hwnd(&self) -> Option<isize> {
        if let Some(hwnd) = self.hwnd.get() {
            return Some(hwnd);
        }
        let found = window::find_app_window();
        self.hwnd.set(found);
        found
    }
}
