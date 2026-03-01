use tauri::State;

use crate::DbState;

#[tauri::command]
pub async fn fix_issue_with_claude(
    state: State<'_, DbState>,
    project_id: String,
    issue_id: String,
) -> Result<String, String> {
    // Gather DB data under the lock, then release before spawning claude
    let (conn_project_id, conn_issue_id) = (project_id.clone(), issue_id.clone());
    let result = {
        let mut conn = state.0.lock().unwrap();
        crate::services::claude_service::fix_issue(&mut conn, &conn_project_id, &conn_issue_id)
    };

    result.map_err(String::from)
}
