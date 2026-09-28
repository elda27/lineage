mod automation;
mod browser;
mod mutation;
mod schedule;
mod skill;

use tauri::Manager;

#[tauri::command]
fn browser_rendered(app: tauri::AppHandle) {
    if let Some(main) = app.get_webview_window("main") {
        _ = main.show();
        _ = main.set_focus();
    }
    if let Some(startup) = app.get_webview_window("startup") {
        _ = startup.close();
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
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
            browser_rendered,
            automation::automation_match,
            automation::automation_run,
            automation::automation_render,
            automation::automation_record,
            automation::credential_set,
            automation::credential_has,
            automation::credential_delete,
            automation::verify_lineage,
            mutation::local_mutation_apply,
            mutation::local_query,
            browser::browser_agent_run,
            schedule::schedule_status,
            schedule::schedule_register,
            schedule::schedule_unregister,
            skill::agent_skill_scan,
            skill::agent_skill_install,
            skill::agent_skill_agentos_path,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
