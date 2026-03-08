use tauri::{AppHandle, State};

use crate::utils::pty::PtyManager;
use crate::SettingsState;

#[tauri::command]
pub async fn pty_spawn(
    app: AppHandle,
    settings_state: State<'_, SettingsState>,
    pty_manager: State<'_, PtyManager>,
    project_path: String,
    cols: u16,
    rows: u16,
) -> Result<u32, String> {
    let settings = settings_state.settings.read().unwrap().clone();

    let args: Vec<&str> = if settings.llm_args.is_empty() {
        vec![]
    } else {
        settings.llm_args.split_whitespace().collect()
    };

    pty_manager
        .spawn(
            &app,
            &settings.llm_command,
            &args,
            &project_path,
            cols,
            rows,
        )
        .await
}

#[tauri::command]
pub async fn pty_write(
    pty_manager: State<'_, PtyManager>,
    session_id: u32,
    data: String,
) -> Result<(), String> {
    pty_manager.write(session_id, &data).await
}

#[tauri::command]
pub async fn pty_resize(
    pty_manager: State<'_, PtyManager>,
    session_id: u32,
    cols: u16,
    rows: u16,
) -> Result<(), String> {
    pty_manager.resize(session_id, cols, rows).await
}

#[tauri::command]
pub async fn pty_kill(pty_manager: State<'_, PtyManager>, session_id: u32) -> Result<(), String> {
    pty_manager.kill(session_id).await
}
