//! OS 連携（Windows）。
//!
//! gpui が面倒を見ない部分——グローバルホットキー、タスクトレイ、
//! 直前にフォアグラウンドだったアプリの観測、ウィンドウの表示/非表示——をここに閉じ込める。
//! 上位層はここが Win32 で実装されていることを知らない。

pub mod foreground;
pub mod launcher;
pub mod single_instance;
pub mod tray;
pub mod tray_promotion;
pub mod window;

pub use foreground::ForegroundApp;

/// 直前のアプリから選択テキストを自動取得する、という要求。
#[derive(Debug, Clone, Copy)]
pub struct SelectionCapture;

/// OS 側から届くイベント。
#[derive(Debug, Clone)]
pub enum SystemEvent {
    /// Alt+Space が押された（表示⇔非表示のトグル）。
    ToggleCapture {
        context: Option<ForegroundApp>,
        selection: Option<SelectionCapture>,
    },
    /// トレイから明示的に表示を要求された。
    ShowCapture { context: Option<ForegroundApp> },
    /// 自動取り込み設定が切り替えられた。
    SetAutoPullForegroundText(bool),
    /// fullos を起動する。
    LaunchFullos,
    /// hash-chain を検証する。
    VerifyLineage,
    /// minos を終了する。
    Quit,
}
