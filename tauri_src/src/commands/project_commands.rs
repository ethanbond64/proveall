use chrono::Utc;
use diesel::prelude::*;
use serde::Serialize;
use tauri::State;

use crate::db::schema::projects;
use crate::models::branch_context::NewBranchContext;
use crate::models::project::{NewProject, ProjectLastOpenedUpdate};
use crate::repositories::{branch_context_repo, project_repo};
use crate::utils::hash_id;
use crate::DbState;

#[derive(Serialize)]
pub struct ProjectListItem {
    pub id: String,
    pub path: String,
}

#[tauri::command]
pub fn fetch_projects(state: State<DbState>, limit: i64) -> Result<Vec<ProjectListItem>, String> {
    let mut conn = state.0.lock().unwrap();
    project_repo::list(&mut conn, |q| {
        q.order(projects::last_opened.desc()).limit(limit)
    })
    .map(|projects| {
        projects
            .into_iter()
            .map(|p| ProjectListItem {
                id: p.id,
                path: p.path,
            })
            .collect()
    })
    .map_err(String::from)
}

#[derive(Serialize)]
pub struct OpenProjectResponse {
    pub id: String,
}

#[tauri::command]
pub fn open_project(state: State<DbState>, path: String) -> Result<OpenProjectResponse, String> {
    let mut conn = state.0.lock().unwrap();

    let existing = project_repo::find_by(&mut conn, |q| q.filter(projects::path.eq(path.clone())))
        .map_err(String::from)?;

    let id = match existing {
        Some(project) => {
            let now = Utc::now().naive_utc();
            let changeset = ProjectLastOpenedUpdate {
                last_opened: Some(now),
            };
            project_repo::update(&mut conn, &project.id, changeset)
                .map_err(String::from)?
                .id
        }
        None => {
            let new_project = NewProject::new(path, None);
            project_repo::create(&mut conn, new_project)
                .map_err(String::from)?
                .id
        }
    };

    Ok(OpenProjectResponse { id })
}

#[derive(Serialize)]
pub struct BranchData {
    pub branch: String,
    pub base_branch: String,
}

#[derive(Serialize)]
pub struct DiffSummary {
    pub files_added: i32,
    pub files_modified: i32,
    pub files_deleted: i32,
    pub lines_added: i32,
    pub lines_deleted: i32,
}

#[derive(Serialize)]
pub struct EventEntry {
    pub id: Option<String>,
    pub event_type: String,
    pub commit: String,
    pub author: Option<String>,
    pub message: String,
    pub created_at: String,
}

#[derive(Serialize)]
pub struct IssueEntry {
    pub id: String,
    pub comment: String,
}

#[derive(Serialize)]
pub struct ProjectStateResponse {
    pub branch_data: BranchData,
    pub diff_summary: DiffSummary,
    pub events: Vec<EventEntry>,
    pub issues: Vec<IssueEntry>,
}

#[tauri::command]
pub fn get_project_state(
    state: State<DbState>,
    project_id: String,
    branch_context_id: String,
) -> Result<ProjectStateResponse, String> {
    let mut conn = state.0.lock().unwrap();
    crate::services::project_service::get_project_state(&mut conn, &project_id, &branch_context_id)
        .map_err(String::from)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateBranchContextResponse {
    pub id: String,
}

#[tauri::command]
pub fn create_branch_context(
    state: State<DbState>,
    project_id: String,
    branch: String,
    base_branch: String,
    settings: String,
) -> Result<CreateBranchContextResponse, String> {
    let mut conn = state.0.lock().unwrap();
    let id = hash_id::branch_context_id(&project_id, &branch, &base_branch);
    // Return existing record if found, otherwise create a new one.
    let bc = match branch_context_repo::get(&mut conn, &id) {
        Ok(bc) => bc,
        Err(_) => branch_context_repo::create(
            &mut conn,
            NewBranchContext::new(project_id, branch, base_branch, settings),
        )
        .map_err(String::from)?,
    };
    Ok(CreateBranchContextResponse { id: bc.id })
}

#[tauri::command]
pub fn get_current_branch(state: State<DbState>, project_id: String) -> Result<String, String> {
    let mut conn = state.0.lock().unwrap();
    let project = project_repo::get(&mut conn, &project_id).map_err(String::from)?;

    crate::utils::git::current_branch(&project.path)
        .map_err(|e| format!("Failed to get current branch: {}", e))
}
