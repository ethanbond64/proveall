use tauri::State;

use crate::{DbState, SettingsState};

#[tauri::command]
pub fn build_issue_prompt(
    db_state: State<'_, DbState>,
    settings_state: State<'_, SettingsState>,
    issue_id: String,
    branch_context_id: String,
) -> Result<String, String> {
    let settings = settings_state.settings.read().unwrap().clone();
    let mut conn = db_state.0.lock().unwrap();
    let ctx = crate::services::llm_service::gather_issue_context(
        &mut conn,
        &issue_id,
        &branch_context_id,
    )
    .map_err(String::from)?;

    Ok(crate::services::llm_service::build_prompt(
        &ctx,
        &settings.llm_prompt_template,
    ))
}
