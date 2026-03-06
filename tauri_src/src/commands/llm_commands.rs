use tauri::State;

use crate::{DbState, SettingsState};

#[tauri::command]
pub async fn fix_issue(
    db_state: State<'_, DbState>,
    settings_state: State<'_, SettingsState>,
    project_id: String,
    issue_id: String,
    branch_context_id: String,
) -> Result<String, String> {
    // Gather DB data under the lock, then release immediately
    let ctx = {
        let mut conn = db_state.0.lock().unwrap();
        crate::services::llm_service::gather_issue_context(
            &mut conn,
            &project_id,
            &issue_id,
            &branch_context_id,
        )
        .map_err(String::from)?
    };

    // Run LLM + commit without holding the DB lock
    let config = settings_state.settings.read().unwrap().to_llm_config();
    crate::services::llm_service::execute_fix(&ctx, &config).map_err(String::from)
}
