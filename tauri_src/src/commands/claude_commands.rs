use tauri::State;

use crate::DbState;

#[tauri::command]
pub async fn fix_issue_with_claude(
    state: State<'_, DbState>,
    project_id: String,
    issue_id: String,
) -> Result<String, String> {
    // Gather DB data under the lock, then release immediately
    let ctx = {
        let mut conn = state.0.lock().unwrap();
        crate::services::claude_service::gather_issue_context(&mut conn, &project_id, &issue_id)
            .map_err(String::from)?
    };

    // Run Claude + commit without holding the DB lock
    crate::services::claude_service::execute_fix(&ctx).map_err(String::from)
}
