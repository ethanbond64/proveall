use tauri::{AppHandle, State};

use crate::utils::pty::PtyManager;

#[tauri::command]
pub async fn pty_spawn(
    app: AppHandle,
    pty_manager: State<'_, PtyManager>,
    project_path: String,
    cols: u16,
    rows: u16,
    command: String,
    args: String,
) -> Result<u32, String> {
    let args_str = args;

    let args: Vec<&str> = if args_str.is_empty() {
        vec![]
    } else {
        args_str.split_whitespace().collect()
    };

    pty_manager
        .spawn(&app, &command, &args, &project_path, cols, rows)
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
