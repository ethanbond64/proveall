use std::path::Path;

use serde::Serialize;
use tauri::State;

use crate::error::AppError;
use crate::DbState;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DirectoryEntry {
    pub name: String,
    pub path: String,
    #[serde(rename = "type")]
    pub entry_type: String,
}

#[tauri::command]
pub fn get_directory(
    state: State<DbState>,
    project_id: String,
    directory_path: String,
) -> Result<Vec<DirectoryEntry>, String> {
    let mut conn = state.0.lock().unwrap();

    let project = crate::repositories::project_repo::get(&mut conn, &project_id)
        .map_err(String::from)?;
    // todo - confirm directory_path will be the relative path from project's root
    let full_path = Path::new(&project.path).join(&directory_path);

    let mut entries: Vec<DirectoryEntry> = std::fs::read_dir(&full_path)
        .map_err(|e| AppError::from(e).to_string())?
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let metadata = entry.metadata().ok()?;
            Some(DirectoryEntry {
                name: entry.file_name().to_string_lossy().into_owned(),
                path: entry.path().to_string_lossy().into_owned(),
                entry_type: if metadata.is_dir() {
                    "dir".to_string()
                } else {
                    "file".to_string()
                },
            })
        })
        .collect();

    entries.sort_by(|a, b| {
        (b.entry_type == "dir")
            .cmp(&(a.entry_type == "dir"))
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });

    Ok(entries)
}
