use serde::{Deserialize, Serialize};
use tauri::State;

use crate::DbState;

// ---------------------------------------------------------------------------
// createEvent types
// ---------------------------------------------------------------------------

#[derive(Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct NewIssueReviewInput {
    #[serde(rename = "type")]
    pub type_: String,
    pub path: Option<String>,
    pub start: Option<i32>,
    pub end: Option<i32>,
    pub state: String,
}

#[derive(Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct NewIssueInput {
    pub comment: String,
    pub reviews: Vec<NewIssueReviewInput>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateEventResponse {
    pub id: String,
}

#[tauri::command]
pub fn create_event(
    state: State<DbState>,
    project_id: String,
    commit: String,
    event_type: String,
    new_issues: Vec<NewIssueInput>,
    resolved_issues: Vec<String>,
    branch_context_id: String,
) -> Result<CreateEventResponse, String> {
    let mut conn = state.0.lock().unwrap();
    crate::services::event_service::create_event(
        &mut conn,
        &project_id,
        commit,
        event_type,
        new_issues,
        resolved_issues,
        &branch_context_id,
    )
    .map_err(String::from)
}
