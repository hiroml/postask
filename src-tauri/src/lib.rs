mod classify;
mod commands;
mod config;
mod db;
mod error;
mod kind;
mod llm;
mod models;

use std::sync::{Arc, Mutex};

use tauri::Manager;

use commands::AppState;
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

            // DB は OS のアプリデータ用フォルダに置く（Q-012）
            let dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&dir)?;
            let conn = db::open(&dir.join(db::DB_FILE_NAME))?;
            db::fail_interrupted(&conn)?;

            app.manage(AppState {
                db: Arc::new(Mutex::new(conn)),
                llm: Arc::new(ClaudeProvider::new(&config)?),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::create_post,
            commands::list_timeline,
            commands::retry_classification,
            commands::list_kinds,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
