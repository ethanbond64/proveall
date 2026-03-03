use tauri::State;

use crate::utils::llm::{save_settings, LlmConfig};
use crate::LlmConfigState;

#[tauri::command]
pub fn get_llm_settings(config_state: State<'_, LlmConfigState>) -> Result<LlmConfig, String> {
    let config = config_state.config.read().unwrap().clone();
    Ok(config)
}

#[tauri::command]
pub fn update_llm_settings(
    config_state: State<'_, LlmConfigState>,
    command: String,
    args: String,
) -> Result<(), String> {
    let new_config = LlmConfig {
        command,
        args,
    };

    // Persist to disk
    save_settings(&config_state.app_data_dir, &new_config).map_err(String::from)?;

    // Update in-memory config
    let mut config = config_state.config.write().unwrap();
    *config = new_config;
    Ok(())
}
