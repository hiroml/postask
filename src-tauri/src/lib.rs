mod classify;
mod commands;
mod config;
mod dashboard;
mod db;
mod error;
mod kind;
mod llm;
mod models;
mod reply;

use std::sync::{Arc, Mutex};

use tauri::Manager;

use commands::{AppInfo, AppState};
use config::Config;
use llm::claude::ClaudeProvider;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let config = Config::load();
            if config.api_key.is_none() {
                eprintln!("ANTHROPIC_API_KEY が未設定です。ポストの分類は失敗扱いになります。");
            }
            eprintln!("分類モデル: {}", config.model);

            // DB は OS のアプリデータ用フォルダに置く（Q-012）。開発用ビルドは別のファイルを使う（仕様004）
            let dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&dir)?;
            let location = db::resolve_db_location(
                std::env::var("X_CLONE_DB").ok().as_deref(),
                cfg!(debug_assertions),
                &dir,
            );
            if location.dev && !location.overridden {
                let prod = dir.join(db::DB_FILE_NAME);
                if db::prepare_dev_db(&location.path, &prod)? {
                    eprintln!(
                        "開発用 DB を普段使いの DB からコピーしました: {}",
                        prod.display()
                    );
                }
            }
            eprintln!("DB: {}", location.path.display());

            let conn = db::open(&location.path)?;
            db::fail_interrupted(&conn)?;

            if location.dev {
                if let Some(window) = app.get_webview_window("main") {
                    window.set_title("x-clone（開発版）")?;
                }
            }

            app.manage(AppState {
                db: Arc::new(Mutex::new(conn)),
                llm: Arc::new(ClaudeProvider::new(&config)?),
                info: AppInfo {
                    dev: location.dev,
                    db_path: location.path.display().to_string(),
                },
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::create_post,
            commands::list_timeline,
            commands::retry_classification,
            commands::set_task_status,
            commands::request_ai_reply,
            commands::list_kinds,
            commands::app_info,
            commands::get_task_dashboard,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
