use tauri::State;

use crate::utils::settings::{default_settings, save_settings, AppSettings};
use crate::SettingsState;

#[tauri::command]
pub fn get_settings(state: State<'_, SettingsState>) -> Result<AppSettings, String> {
    let settings = state.settings.read().unwrap().clone();
    Ok(settings)
}

#[tauri::command]
pub fn set_settings(state: State<'_, SettingsState>, settings: AppSettings) -> Result<(), String> {
    save_settings(&state.app_data_dir, &settings).map_err(String::from)?;

    let mut current = state.settings.write().unwrap();
    *current = settings;
    Ok(())
}

#[tauri::command]
pub fn reset_settings(state: State<'_, SettingsState>) -> Result<AppSettings, String> {
    let defaults = default_settings();

    save_settings(&state.app_data_dir, &defaults).map_err(String::from)?;

    let mut current = state.settings.write().unwrap();
    *current = defaults.clone();
    Ok(defaults)
}
