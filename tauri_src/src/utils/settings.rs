use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::AppError;

fn default_llm_command() -> String {
    "claude".to_string()
}

fn default_llm_args() -> String {
    String::new()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    #[serde(default = "default_llm_command", alias = "command")]
    pub llm_command: String,
    #[serde(default = "default_llm_args", alias = "args")]
    pub llm_args: String,
    #[serde(default)]
    pub auto_update: bool,
}

pub fn default_settings() -> AppSettings {
    AppSettings {
        llm_command: default_llm_command(),
        llm_args: default_llm_args(),
        auto_update: false,
    }
}

fn settings_path(app_data_dir: &Path) -> PathBuf {
    app_data_dir.join("settings.json")
}

pub fn load_settings(app_data_dir: &Path) -> AppSettings {
    let path = settings_path(app_data_dir);
    match std::fs::read_to_string(&path) {
        Ok(contents) => serde_json::from_str(&contents).unwrap_or_else(|_| default_settings()),
        Err(_) => default_settings(),
    }
}

pub fn save_settings(app_data_dir: &Path, settings: &AppSettings) -> Result<(), AppError> {
    let path = settings_path(app_data_dir);
    let json = serde_json::to_string_pretty(settings)
        .map_err(|e| AppError::Io(std::io::Error::other(e)))?;
    std::fs::write(&path, json)?;
    Ok(())
}
