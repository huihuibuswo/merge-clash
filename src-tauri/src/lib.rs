mod commands;
mod database;
mod error;
mod fetcher;
mod merge;
mod models;
mod publisher;
mod templates;

use commands::AppState;
use std::sync::Arc;
use tauri::Manager;
use tokio::sync::Mutex;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let data_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
            let db_path = data_dir.join("merge-clash.db");
            let db = tauri::async_runtime::block_on(database::Database::open(&db_path))
                .map_err(|e| e.to_string())?;
            let http = fetcher::client().map_err(|e| e.to_string())?;
            app.manage(AppState {
                db,
                http,
                publisher: Arc::new(Mutex::new(None)),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_templates,
            commands::get_project_settings,
            commands::select_project_template,
            commands::save_theme,
            commands::list_subscriptions,
            commands::save_subscription,
            commands::delete_subscription,
            commands::test_subscription_url,
            commands::test_subscription,
            commands::refresh_subscriptions,
            commands::get_draft,
            commands::save_proxy_groups,
            commands::save_draft_yaml,
            commands::list_draft_history,
            commands::restore_draft_history,
            commands::delete_draft_history,
            commands::delete_other_draft_history,
            commands::publish_draft,
            commands::list_published_versions,
            commands::activate_published_version,
            commands::delete_published_version,
            commands::delete_other_published_versions,
            commands::get_publish_status,
            commands::start_publish_server,
            commands::stop_publish_server,
            commands::save_publish_settings,
            commands::rotate_publish_token,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Merge Clash");
}
