use portable_pty::{native_pty_system, CommandBuilder, MasterPty, PtySize};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use tokio::sync::Mutex as AsyncMutex;

use base64::Engine;

pub struct PtySession {
    writer: Arc<AsyncMutex<Box<dyn Write + Send>>>,
    master: Arc<AsyncMutex<Box<dyn MasterPty + Send>>>,
    child: Arc<AsyncMutex<Box<dyn portable_pty::Child + Send>>>,
}

pub struct PtyManager {
    sessions: AsyncMutex<HashMap<u32, PtySession>>,
    next_id: AsyncMutex<u32>,
}

impl PtyManager {
    pub fn new() -> Self {
        Self {
            sessions: AsyncMutex::new(HashMap::new()),
            next_id: AsyncMutex::new(1),
        }
    }

    pub async fn spawn(
        &self,
        app: &AppHandle,
        command: &str,
        args: &[&str],
        cwd: &str,
        cols: u16,
        rows: u16,
    ) -> Result<u32, String> {
        let pty_system = native_pty_system();

        let pair = pty_system
            .openpty(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| format!("Failed to open PTY: {e}"))?;

        let mut cmd = CommandBuilder::new(command);
        cmd.args(args);
        cmd.cwd(cwd);

        // Inherit PATH from current process (already fixed by fix_path_env)
        if let Ok(path) = std::env::var("PATH") {
            cmd.env("PATH", path);
        }
        if let Ok(home) = std::env::var("HOME") {
            cmd.env("HOME", home);
        }
        cmd.env("TERM", "xterm-256color");

        let child = pair
            .slave
            .spawn_command(cmd)
            .map_err(|e| format!("Failed to spawn command: {e}"))?;

        let reader = pair
            .master
            .try_clone_reader()
            .map_err(|e| format!("Failed to clone reader: {e}"))?;
        let writer = pair
            .master
            .take_writer()
            .map_err(|e| format!("Failed to take writer: {e}"))?;

        let id = {
            let mut next = self.next_id.lock().await;
            let id = *next;
            *next += 1;
            id
        };

        // Offload the blocking read loop to a dedicated thread via
        // spawn_blocking so we don't starve the tokio async runtime.
        let app_handle = app.clone();
        let session_id = id;
        tokio::task::spawn_blocking(move || {
            Self::read_loop(reader, app_handle, session_id);
        });

        {
            let mut sessions = self.sessions.lock().await;
            sessions.insert(
                id,
                PtySession {
                    writer: Arc::new(AsyncMutex::new(writer)),
                    master: Arc::new(AsyncMutex::new(pair.master)),
                    child: Arc::new(AsyncMutex::new(child)),
                },
            );
        }

        Ok(id)
    }

    fn read_loop(mut reader: Box<dyn Read + Send>, app: AppHandle, session_id: u32) {
        let mut buf = [0u8; 4096];
        let engine = base64::engine::general_purpose::STANDARD;
        loop {
            match reader.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    // Encode as base64 to avoid splitting multi-byte UTF-8
                    // sequences across event boundaries
                    let data = engine.encode(&buf[..n]);
                    let _ = app.emit(&format!("pty-output-{session_id}"), data);
                }
                Err(_) => break,
            }
        }
        let _ = app.emit(&format!("pty-exit-{session_id}"), ());
    }

    pub async fn write(&self, id: u32, data: &str) -> Result<(), String> {
        let sessions = self.sessions.lock().await;
        let session = sessions
            .get(&id)
            .ok_or_else(|| "PTY session not found".to_string())?;
        let writer = session.writer.clone();
        drop(sessions);

        let data = data.to_owned();
        tokio::task::spawn_blocking(move || {
            let mut writer = writer.blocking_lock();
            writer
                .write_all(data.as_bytes())
                .map_err(|e| format!("Failed to write to PTY: {e}"))?;
            writer
                .flush()
                .map_err(|e| format!("Failed to flush PTY: {e}"))?;
            Ok(())
        })
        .await
        .map_err(|e| format!("Task join error: {e}"))?
    }

    pub async fn resize(&self, id: u32, cols: u16, rows: u16) -> Result<(), String> {
        let sessions = self.sessions.lock().await;
        let session = sessions
            .get(&id)
            .ok_or_else(|| "PTY session not found".to_string())?;
        let master = session.master.clone();
        drop(sessions);

        let master = master.lock().await;
        master
            .resize(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| format!("Failed to resize PTY: {e}"))?;
        Ok(())
    }

    pub async fn kill(&self, id: u32) -> Result<(), String> {
        let mut sessions = self.sessions.lock().await;
        if let Some(session) = sessions.remove(&id) {
            let mut child = session.child.lock().await;
            let _ = child.kill();
        }
        Ok(())
    }
}
