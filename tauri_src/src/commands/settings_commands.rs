use tauri::State;

use crate::utils::llm::{default_llm_config, save_settings, LlmConfig};
use crate::LlmState;

#[tauri::command]
pub fn get_llm_settings(llm_state: State<'_, LlmState>) -> Result<LlmConfig, String> {
    let config = llm_state.config.read().unwrap().clone();
    Ok(config)
}

#[tauri::command]
pub fn update_llm_settings(
    llm_state: State<'_, LlmState>,
    command: String,
    args: String,
) -> Result<(), String> {
    let new_config = LlmConfig { command, args };

    // Persist to disk
    save_settings(&llm_state.app_data_dir, &new_config).map_err(String::from)?;

    // Update in-memory config
    let mut config = llm_state.config.write().unwrap();
    *config = new_config;
    Ok(())
}

#[tauri::command]
pub fn reset_llm_settings(llm_state: State<'_, LlmState>) -> Result<LlmConfig, String> {
    let defaults = default_llm_config();

    save_settings(&llm_state.app_data_dir, &defaults).map_err(String::from)?;

    let mut config = llm_state.config.write().unwrap();
    *config = defaults.clone();
    Ok(defaults)
}
