// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        // 記録の読み出しは webview 側（core/infrastructure/persistence/sqlite）が行う。
        // Rust 側は SQLite ハンドルを渡すだけで、SQL は持たない。
        .plugin(tauri_plugin_sql::Builder::default().build())
        .setup(|app| {
            // GitHub Release の latest.json を見に行く自動更新。
            // エンドポイントと公開鍵は tauri.conf.json の plugins.updater。
            #[cfg(desktop)]
            {
                app.handle()
                    .plugin(tauri_plugin_updater::Builder::new().build())?;
                app.handle().plugin(tauri_plugin_process::init())?;
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![greet])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
