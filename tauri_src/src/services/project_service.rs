use std::collections::{HashMap, HashSet};

use diesel::prelude::*;
use diesel::sqlite::SqliteConnection;

use crate::commands::project_commands::{
    BranchData, DiffSummary, EventEntry, IssueEntry, ProjectStateResponse,
};
use crate::db::schema::{event_issue_composite_xref, events};
use crate::error::AppError;
use crate::repositories::{branch_context_repo, event_repo, issue_repo, project_repo};
use crate::utils::git::{self, diff_changed_files, parse_stat};

#[cfg(test)]
mod tests;

pub fn get_project_state(
    conn: &mut SqliteConnection,
    project_id: &str,
    branch_context_id: &str,
) -> Result<ProjectStateResponse, AppError> {
    let project = project_repo::get(conn, project_id)?;
    let path = &project.path;

    let current_branch = current_branch(path)?;
    let bc = branch_context_repo::get(conn, branch_context_id)?;
    let base_branch = bc.base_branch.clone();

    if current_branch == base_branch {
        return Ok(ProjectStateResponse {
            branch_data: BranchData {
                branch: current_branch,
                base_branch,
            },
            diff_summary: DiffSummary {
                files_added: 0,
                files_modified: 0,
                files_deleted: 0,
                lines_added: 0,
                lines_deleted: 0,
            },
            events: Vec::new(),
            issues: Vec::new(),
        });
    }

    let range = format!("{}..HEAD", base_branch);
    let diff_summary = build_diff_summary(path, &range)?;
    let event_entries = build_event_entries(conn, path, &range, project_id)?;
    let issue_entries = build_issue_entries(conn, bc.head_event_id.as_deref(), branch_context_id)?;

    Ok(ProjectStateResponse {
        branch_data: BranchData {
            branch: current_branch,
            base_branch,
        },
        diff_summary,
        events: event_entries,
        issues: issue_entries,
    })
}

fn current_branch(path: &str) -> Result<String, AppError> {
    git::current_branch(path)
}

fn build_diff_summary(path: &str, range: &str) -> Result<DiffSummary, AppError> {
    let diff_files = diff_changed_files(path, &[range]).unwrap_or_default();
    let files_added = diff_files.iter().filter(|f| f.status == "A").count() as i32;
    let files_modified = diff_files.iter().filter(|f| f.status == "M").count() as i32;
    let files_deleted = diff_files.iter().filter(|f| f.status == "D").count() as i32;

    let shortstat_output = git::diff_shortstat(path, range).unwrap_or_default();

    let lines_added = parse_stat(&shortstat_output, "insertion") as i32;
    let lines_deleted = parse_stat(&shortstat_output, "deletion") as i32;

    Ok(DiffSummary {
        files_added,
        files_modified,
        files_deleted,
        lines_added,
        lines_deleted,
    })
}

fn build_event_entries(
    conn: &mut SqliteConnection,
    path: &str,
    range: &str,
    project_id: &str,
) -> Result<Vec<EventEntry>, AppError> {
    struct CommitRecord {
        hash: String,
        message: String,
        author: String,
        date: String,
    }

    let format_arg = "--format=%H%n%s%n%an%n%ae%n%aI";
    let log_output = git::log_commits(path, format_arg, range).unwrap_or_default();

    let log_lines: Vec<&str> = log_output.lines().collect();
    let commits: Vec<CommitRecord> = log_lines
        .chunks(5)
        .filter(|chunk: &&[&str]| chunk.len() == 5)
        .map(|chunk| CommitRecord {
            hash: chunk[0].to_string(),
            message: chunk[1].to_string(),
            author: chunk[2].to_string(),
            date: chunk[4].to_string(),
        })
        .collect();

    let commit_hashes: Vec<String> = commits.iter().map(|c| c.hash.clone()).collect();

    let project_id_owned = project_id.to_string();
    let mut events_by_hash: HashMap<String, Vec<_>> = HashMap::new();
    for e in event_repo::list(conn, |q| {
        q.filter(
            events::project_id
                .eq(project_id_owned)
                .and(events::hash.eq_any(commit_hashes)),
        )
    })? {
        if let Some(h) = e.hash.clone() {
            events_by_hash.entry(h).or_default().push(e);
        }
    }

    let entries: Vec<EventEntry> = commits
        .iter()
        .flat_map(|commit| {
            if let Some(evs) = events_by_hash.get_mut(&commit.hash) {
                evs.sort_by(|a, b| b.created_at.cmp(&a.created_at));
                evs.iter()
                    .map(|ev| EventEntry {
                        id: Some(ev.id.clone()),
                        event_type: ev.type_.clone(),
                        commit: ev.hash.clone().unwrap_or_default(),
                        author: Some(commit.author.clone()),
                        message: ev.summary.clone(),
                        created_at: ev.created_at.to_string(),
                    })
                    .collect::<Vec<_>>()
            } else {
                vec![EventEntry {
                    id: None,
                    event_type: "commit".to_string(),
                    commit: commit.hash.clone(),
                    author: Some(commit.author.clone()),
                    message: commit.message.clone(),
                    created_at: commit.date.clone(),
                }]
            }
        })
        .collect();

    Ok(entries)
}

fn build_issue_entries(
    conn: &mut SqliteConnection,
    head_event_id: Option<&str>,
    branch_context_id: &str,
) -> Result<Vec<IssueEntry>, AppError> {
    let Some(event_id) = head_event_id else {
        return Ok(Vec::new());
    };

    let event_id_owned = event_id.to_string();
    let bc_id_owned = branch_context_id.to_string();
    let mut seen = HashSet::new();
    Ok(issue_repo::join_list(conn, |q| {
        q.filter(
            event_issue_composite_xref::event_id
                .eq(event_id_owned)
                .and(event_issue_composite_xref::branch_context_id.eq(bc_id_owned)),
        )
    })?
    .into_iter()
    .filter(|(_, issue)| seen.insert(issue.id.clone()))
    .map(|(_, issue)| IssueEntry {
        id: issue.id,
        comment: issue.comment,
    })
    .collect())
}
