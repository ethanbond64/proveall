use std::sync::Mutex;

use crate::commands::event_commands::create_event;
use crate::commands::fs_commands::get_directory;
use crate::commands::project_commands::{
    create_branch_context, fetch_projects, get_current_branch, get_project_state, open_project,
};
use crate::commands::review_commands::{get_review_file_data, get_review_file_system_data};
use diesel::sqlite::SqliteConnection;

mod commands;
mod db;
mod error;
mod models;
mod repositories;
mod services;
mod utils;

pub struct DbState(pub Mutex<SqliteConnection>);

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Get the platform-specific application data directory
    // This is where desktop apps should store user data:
    // - macOS: ~/Library/Application Support/com.proveall/
    // - Windows: %APPDATA%/com.proveall/
    // - Linux: ~/.local/share/com.proveall/
    let app_data_dir = dirs_next::data_dir()
        .expect("Failed to resolve app data directory")
        .join("com.proveall");

    // Ensure the directory exists (created on first run)
    std::fs::create_dir_all(&app_data_dir).expect("Failed to create app data directory");

    // Database file will be stored in the app data directory
    // This persists across app launches and is user-specific
    let db_path = app_data_dir.join("proveall.db");
    let db_path_str = db_path.to_str().expect("Invalid db path");

    // Establish connection and run migrations
    let conn = db::connection::establish_connection(db_path_str);

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(DbState(Mutex::new(conn)))
        .invoke_handler(tauri::generate_handler![
            fetch_projects,
            open_project,
            get_project_state,
            get_current_branch,
            create_branch_context,
            create_event,
            get_review_file_system_data,
            get_review_file_data,
            get_directory,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
