mod automation;
mod browser;
mod mutation;
mod schedule;
mod skill;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        // WebView は plugin-sql を読み取り専用で使う。INSERT / UPDATE / DELETE は
        // mutation.rs の Tauri command を通して Rust 側へ集約する。
        // lineage(links) への追記を伴う自動化結果だけは、従来どおり agentos に委ねる。
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
            mutation::memo_set_done,
            mutation::memo_set_archived,
            mutation::memo_trash,
            mutation::memo_archive_done,
            mutation::automation_rule_save,
            mutation::automation_rule_delete,
            mutation::setting_set,
            mutation::tag_update,
            mutation::tag_delete,
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
