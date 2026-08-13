mod automation;
mod browser;
mod schedule;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        // 記録の読み出しは webview 側（core/infra/persistence/sqlite）が行う。
        // Rust 側は SQLite ハンドルを渡すだけで、SQL は持たない。
        //
        // ただし自動化だけは例外で、lineage(links) への追記を伴うため同梱の agentos に
        // 委ねる（automation.rs）。webview から書けてしまうと、hash-chain の作り方が
        // minos / agentos / fullos で分岐しうる。
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
        .invoke_handler(tauri::generate_handler![
            automation::automation_match,
            automation::automation_run,
            automation::automation_render,
            automation::automation_record,
            automation::credential_set,
            automation::credential_has,
            automation::credential_delete,
            automation::verify_lineage,
            browser::browser_agent_run,
            schedule::schedule_status,
            schedule::schedule_register,
            schedule::schedule_unregister,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
