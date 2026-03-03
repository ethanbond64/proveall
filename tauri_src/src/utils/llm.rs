use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde::{Deserialize, Serialize};

use crate::error::AppError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmConfig {
    pub command: String,
    pub args: String,
}

pub fn default_llm_config() -> LlmConfig {
    LlmConfig {
        command: "claude".to_string(),
        args: "--print --dangerously-skip-permissions --allowedTools Edit,Read,Write,Grep,Glob"
            .to_string(),
    }
}

pub fn call_llm(config: &LlmConfig, project_path: &str, prompt: &str) -> Result<String, AppError> {
    let args: Vec<&str> = config.args.split_whitespace().collect();

    let mut child = Command::new(&config.command)
        .args(&args)
        .current_dir(project_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(prompt.as_bytes())?;
    }

    let output = child.wait_with_output()?;
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();

    if output.status.success() {
        Ok(stdout)
    } else {
        Err(AppError::Llm(stderr))
    }
}

fn settings_path(app_data_dir: &Path) -> PathBuf {
    app_data_dir.join("settings.json")
}

pub fn load_settings(app_data_dir: &Path) -> LlmConfig {
    let path = settings_path(app_data_dir);
    match std::fs::read_to_string(&path) {
        Ok(contents) => serde_json::from_str(&contents).unwrap_or_else(|_| default_llm_config()),
        Err(_) => default_llm_config(),
    }
}

pub fn save_settings(app_data_dir: &Path, config: &LlmConfig) -> Result<(), AppError> {
    let path = settings_path(app_data_dir);
    let json = serde_json::to_string_pretty(config)
        .map_err(|e| AppError::Io(std::io::Error::other(e)))?;
    std::fs::write(&path, json)?;
    Ok(())
}
