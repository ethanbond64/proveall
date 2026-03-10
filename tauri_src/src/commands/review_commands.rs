use serde::{Deserialize, Serialize};
use tauri::State;

use crate::DbState;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TouchedFile {
    pub name: String,
    pub path: String,
    pub diff_mode: Option<String>,
    pub state: String,
    pub merge_only: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewIssueEntry {
    pub id: String,
    pub comment: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewFileSystemDataResponse {
    pub touched_files: Vec<TouchedFile>,
    pub issues: Vec<ReviewIssueEntry>,
}

#[tauri::command]
pub fn get_review_file_system_data(
    state: State<DbState>,
    project_id: String,
    commit: String,
    issue_id: Option<String>,
    review_type: String,
    branch_context_id: String,
) -> Result<ReviewFileSystemDataResponse, String> {
    let mut conn = state.0.lock().unwrap();
    crate::services::review_service::get_review_file_system_data(
        &mut conn,
        &project_id,
        commit,
        issue_id.as_deref(),
        review_type,
        &branch_context_id,
    )
    .map_err(String::from)
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct LineSummaryEntry {
    pub start: i32,
    pub end: i32,
    pub state: String,
    pub issue_id: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewFileDataResponse {
    pub content: String,
    pub diff: Option<String>,
    pub line_summary: Vec<LineSummaryEntry>,
    pub issues: Vec<ReviewIssueEntry>,
}

#[tauri::command]
pub fn get_review_file_data(
    state: State<DbState>,
    project_id: String,
    commit: String,
    issue_id: Option<String>,
    review_type: String,
    file_path: String,
    branch_context_id: String,
) -> Result<ReviewFileDataResponse, String> {
    let mut conn = state.0.lock().unwrap();
    crate::services::review_service::get_review_file_data(
        &mut conn,
        &project_id,
        commit,
        issue_id.as_deref(),
        review_type,
        file_path,
        &branch_context_id,
    )
    .map_err(String::from)
}
