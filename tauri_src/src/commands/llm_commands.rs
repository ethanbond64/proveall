use tauri::State;

use crate::{DbState, LlmState};

#[tauri::command]
pub async fn fix_issue(
    db_state: State<'_, DbState>,
    llm_state: State<'_, LlmState>,
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
    let config = llm_state.config.read().unwrap().clone();
    crate::services::llm_service::execute_fix(&ctx, &config).map_err(String::from)
}
