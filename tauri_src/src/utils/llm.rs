use std::path::{Path, PathBuf};

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
        args: String::new(),
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
    let json =
        serde_json::to_string_pretty(config).map_err(|e| AppError::Io(std::io::Error::other(e)))?;
    std::fs::write(&path, json)?;
    Ok(())
}
