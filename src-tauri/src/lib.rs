// TODO(001-T5): コマンドから使うようになったら dead_code の許可を外す
#![allow(dead_code)]

mod classify;
mod commands;
mod config;
mod db;
mod error;
mod kind;
mod llm;
mod models;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
