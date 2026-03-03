use portable_pty::{native_pty_system, CommandBuilder, MasterPty, PtySize, SlavePty};
use std::io::{BufRead, BufReader, Read, Write};
use std::sync::Arc;
use tokio::sync::Mutex as AsyncMutex;

pub struct PtyState {
    pub master: Arc<AsyncMutex<Box<dyn MasterPty + Send>>>,
    pub slave: Arc<AsyncMutex<Box<dyn SlavePty + Send>>>,
    pub writer: Arc<AsyncMutex<Box<dyn Write + Send>>>,
    pub reader: Arc<AsyncMutex<BufReader<Box<dyn Read + Send>>>>,
}

#[tauri::command]
pub async fn async_create_shell(
    state: tauri::State<'_, PtyState>,
    project_path: Option<String>,
) -> Result<(), String> {
    let slave = state.slave.lock().await;

    let mut cmd = CommandBuilder::new_default_prog();
    cmd.env("TERM", "xterm-256color");

    if let Some(path) = project_path {
        cmd.cwd(path);
    }

    let mut child = slave.spawn_command(cmd).map_err(|e| e.to_string())?;

    // Monitor exit in background
    std::thread::spawn(move || {
        let _exit = child.wait();
    });

    Ok(())
}

#[tauri::command]
pub async fn async_write_to_pty(
    state: tauri::State<'_, PtyState>,
    data: String,
) -> Result<(), String> {
    let mut writer = state.writer.lock().await;
    write!(writer, "{}", data).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn async_read_from_pty(state: tauri::State<'_, PtyState>) -> Result<String, String> {
    let mut reader = state.reader.lock().await;

    let buf = reader.fill_buf().map_err(|e| e.to_string())?;
    let len = buf.len();
    let result = String::from_utf8_lossy(buf).to_string();
    reader.consume(len);

    Ok(result)
}

#[tauri::command]
pub async fn async_resize_pty(
    state: tauri::State<'_, PtyState>,
    rows: u16,
    cols: u16,
) -> Result<(), String> {
    let master = state.master.lock().await;
    master
        .resize(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn create_pty_state() -> PtyState {
    let pty_system = native_pty_system();
    let pair = pty_system
        .openpty(PtySize {
            rows: 24,
            cols: 80,
            pixel_width: 0,
            pixel_height: 0,
        })
        .expect("Failed to open PTY");

    let writer = pair
        .master
        .take_writer()
        .expect("Failed to take PTY writer");
    let reader = pair
        .master
        .try_clone_reader()
        .expect("Failed to clone PTY reader");

    PtyState {
        master: Arc::new(AsyncMutex::new(pair.master)),
        slave: Arc::new(AsyncMutex::new(pair.slave)),
        writer: Arc::new(AsyncMutex::new(writer)),
        reader: Arc::new(AsyncMutex::new(BufReader::new(reader))),
    }
}
