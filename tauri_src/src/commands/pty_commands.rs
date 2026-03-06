use tauri::{AppHandle, State};

use crate::utils::pty::PtyManager;
use crate::LlmState;

#[tauri::command]
pub fn pty_spawn(
    app: AppHandle,
    llm_state: State<'_, LlmState>,
    pty_manager: State<'_, PtyManager>,
    project_path: String,
    cols: u16,
    rows: u16,
) -> Result<u32, String> {
    let config = llm_state.config.read().unwrap().clone();

    // Launch claude with no args — just an interactive session
    let args: Vec<&str> = vec![];

    pty_manager.spawn(&app, &config.command, &args, &project_path, cols, rows)
}

#[tauri::command]
pub fn pty_write(
    pty_manager: State<'_, PtyManager>,
    session_id: u32,
    data: String,
) -> Result<(), String> {
    pty_manager.write(session_id, &data)
}

#[tauri::command]
pub fn pty_resize(
    pty_manager: State<'_, PtyManager>,
    session_id: u32,
    cols: u16,
    rows: u16,
) -> Result<(), String> {
    pty_manager.resize(session_id, cols, rows)
}

#[tauri::command]
pub fn pty_kill(pty_manager: State<'_, PtyManager>, session_id: u32) -> Result<(), String> {
    pty_manager.kill(session_id)
}
