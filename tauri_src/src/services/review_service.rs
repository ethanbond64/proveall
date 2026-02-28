use std::collections::HashSet;
use std::path::Path;

use diesel::prelude::*;
use diesel::sqlite::SqliteConnection;

use crate::commands::review_commands::{
    LineSummaryEntry, ReviewFileDataResponse, ReviewFileSystemDataResponse, ReviewIssueEntry,
    TouchedFile,
};
use crate::db::schema::{event_issue_composite_xref, events};
use crate::error::AppError;
use crate::models::composite_file_review_state::ReviewSummaryMetadataEntry;
use crate::repositories::{
    branch_context_repo, event_issue_composite_xref_repo, event_repo, issue_repo, project_repo,
};
use crate::utils::git::{diff_changed_files, run_git};

enum ReviewType {
    Commit,
    Branch,
}

impl ReviewType {
    fn parse(s: &str) -> Result<Self, AppError> {
        match s {
            "commit" => Ok(ReviewType::Commit),
            "branch" => Ok(ReviewType::Branch),
            _ => Err(AppError::NotFound(format!("Unknown review type: {}", s))),
        }
    }
}

pub fn get_review_file_system_data(
    conn: &mut SqliteConnection,
    project_id: &str,
    commit: String,
    issue_id: Option<&str>,
    review_type: String,
    branch_context_id: &str,
) -> Result<ReviewFileSystemDataResponse, AppError> {
    let project = project_repo::get(conn, project_id)?;
    let path = &project.path;
    let review_type = ReviewType::parse(&review_type)?;
    let branch_context = branch_context_repo::get(conn, branch_context_id)?;

    let event = find_event_for_commit(conn, project_id, &commit)?;
    let event_id = event.as_ref().map(|e| e.id.as_str());

    let (touched_files, issues) = match review_type {
        ReviewType::Commit => (
            build_commit_touched_files(path, &commit, &event)?,
            build_issues_from_event(
                conn,
                branch_context.head_event_id.as_deref(),
                None,
                branch_context_id,
            )?,
        ),
        ReviewType::Branch => (
            build_branch_touched_files(
                conn,
                path,
                event_id,
                branch_context_id,
                &branch_context.base_branch,
                issue_id,
            )?,
            build_issues_from_event(conn, event_id, None, branch_context_id)?,
        ),
    };

    Ok(ReviewFileSystemDataResponse {
        touched_files,
        issues,
    })
}

fn build_commit_touched_files(
    project_path: &str,
    commit: &str,
    event: &Option<crate::models::event::Event>,
) -> Result<Vec<TouchedFile>, AppError> {
    // If an event already exists for this commit, error
    if event.is_some() {
        return Err(AppError::Git(
            "Unimplemented: commit event already exists for this hash".to_string(),
        ));
    }

    // Files from commit vs its parent, all default to red
    let parent = format!("{}^", commit);
    let diff_files = diff_changed_files(project_path, &[&parent, commit])?;

    Ok(diff_files
        .into_iter()
        .map(|f| TouchedFile {
            name: extract_file_name(&f.path),
            path: f.path,
            diff_mode: Some(f.status),
            state: "red".to_string(),
        })
        .collect())
}

fn build_branch_touched_files(
    conn: &mut SqliteConnection,
    project_path: &str,
    event_id: Option<&str>,
    branch_context_id: &str,
    base_branch: &str,
    issue_id: Option<&str>,
) -> Result<Vec<TouchedFile>, AppError> {
    // Always get all files from git diff
    let range = format!("{}..HEAD", base_branch);
    let diff_files = diff_changed_files(project_path, &[&range])?;

    // Build set of file paths that have a CompositeFileReviewState in the xref
    // Filter by issue_id if provided
    let composite_paths: HashSet<String> = if let Some(eid) = event_id {
        let composites = if let Some(iid) = issue_id {
            // Filter by specific issue
            event_issue_composite_xref_repo::join_list_by_event_and_issue(
                conn,
                eid,
                iid,
                branch_context_id,
            )?
        } else {
            // Get all composites for this event
            event_issue_composite_xref_repo::join_list_by_event(conn, eid, branch_context_id)?
        };

        composites
            .into_iter()
            .map(|(_, composite)| composite.relative_file_path)
            .collect()
    } else {
        HashSet::new()
    };

    // State should be green if no composite exists for the file path, else red
    Ok(diff_files
        .into_iter()
        .map(|f| {
            let state = if composite_paths.contains(&f.path) {
                "red"
            } else {
                "green"
            }
            .to_string();

            TouchedFile {
                name: extract_file_name(&f.path),
                path: f.path,
                diff_mode: Some(f.status),
                state,
            }
        })
        .collect())
}

fn extract_file_name(file_path: &str) -> String {
    Path::new(file_path)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

// ---------------------------------------------------------------------------
// getReviewFileData
// ---------------------------------------------------------------------------

pub fn get_review_file_data(
    conn: &mut SqliteConnection,
    project_id: &str,
    commit: String,
    issue_id: Option<&str>,
    review_type: String,
    file_path: String,
    branch_context_id: &str,
) -> Result<ReviewFileDataResponse, AppError> {
    let project = project_repo::get(conn, project_id)?;
    let path = &project.path;
    let review_type = ReviewType::parse(&review_type)?;
    let branch_context = branch_context_repo::get(conn, branch_context_id)?;

    let event = find_event_for_commit(conn, project_id, &commit)?;
    let event_id = event.as_ref().map(|e| e.id.as_str());

    // Content: file at the commit (same for all review types)
    let ref_path = format!("{}:{}", commit, file_path);
    let content = run_git(path, &["show", &ref_path])
        .map(|o| o.stdout)
        .unwrap_or_default();

    let (diff, line_summary, issues) = match review_type {
        ReviewType::Commit => {
            if event.is_some() {
                return Err(AppError::Git(
                    "Unimplemented: commit event already exists for this hash".to_string(),
                ));
            }
            let parent_ref = format!("{}^:{}", commit, file_path);
            let diff = Some(
                run_git(path, &["show", &parent_ref])
                    .map(|o| o.stdout)
                    .unwrap_or_default(),
            );
            let line_summary = build_branch_line_summary(
                conn,
                branch_context.head_event_id.as_deref(),
                &file_path,
                branch_context_id,
                None,
            )?;
            let issues = build_issues_from_event(
                conn,
                branch_context.head_event_id.as_deref(),
                Some(&file_path),
                branch_context_id,
            )?;
            (diff, line_summary, issues)
        }
        ReviewType::Branch => {
            let base_ref = format!("{}:{}", branch_context.base_branch, file_path);
            let diff = Some(
                run_git(path, &["show", &base_ref])
                    .map(|o| o.stdout)
                    .unwrap_or_default(),
            );
            let line_summary =
                build_branch_line_summary(conn, event_id, &file_path, branch_context_id, issue_id)?;
            let issues =
                build_issues_from_event(conn, event_id, Some(&file_path), branch_context_id)?;
            (diff, line_summary, issues)
        }
    };

    Ok(ReviewFileDataResponse {
        content,
        diff,
        line_summary,
        issues,
    })
}

fn find_event_for_commit(
    conn: &mut SqliteConnection,
    project_id: &str,
    commit: &str,
) -> Result<Option<crate::models::event::Event>, AppError> {
    let commit = commit.to_string();
    let project_id = project_id.to_string();
    Ok(event_repo::list(conn, |q| {
        q.filter(
            events::project_id
                .eq(project_id)
                .and(events::hash.eq(commit)),
        )
        .order(events::created_at.desc())
    })?
    .into_iter()
    .next())
}

fn build_branch_line_summary(
    conn: &mut SqliteConnection,
    event_id: Option<&str>,
    file_path: &str,
    branch_context_id: &str,
    issue_id: Option<&str>,
) -> Result<Vec<LineSummaryEntry>, AppError> {
    let Some(eid) = event_id else {
        return Ok(vec![]);
    };

    // Conditionally filter by issue_id if provided
    let xrefs_with_composites: Vec<_> = if let Some(iid) = issue_id {
        // Filter by specific issue
        event_issue_composite_xref_repo::join_list_by_event_and_issue(
            conn,
            eid,
            iid,
            branch_context_id,
        )?
        .into_iter()
        .filter(|(_, composite)| composite.relative_file_path == file_path)
        .collect()
    } else {
        // Get all composites for this event
        event_issue_composite_xref_repo::join_list_by_event(conn, eid, branch_context_id)?
            .into_iter()
            .filter(|(_, composite)| composite.relative_file_path == file_path)
            .collect()
    };

    let mut line_summary = Vec::new();

    for (xref, composite) in &xrefs_with_composites {
        let entries: Vec<ReviewSummaryMetadataEntry> =
            serde_json::from_str(&composite.summary_metadata).unwrap_or_default();

        for entry in entries {
            line_summary.push(LineSummaryEntry {
                start: entry.start,
                end: entry.end,
                state: entry.state,
                issue_id: xref.issue_id.clone(),
            });
        }
    }

    Ok(line_summary)
}

fn build_issues_from_event(
    conn: &mut SqliteConnection,
    event_id: Option<&str>,
    file_path: Option<&str>,
    branch_context_id: &str,
) -> Result<Vec<ReviewIssueEntry>, AppError> {
    let Some(eid) = event_id else {
        return Ok(vec![]);
    };

    let mut seen = HashSet::new();

    // If file_path is provided, filter by both event_id and file_path
    if let Some(fp) = file_path {
        Ok(
            event_issue_composite_xref_repo::join_list_by_event(conn, eid, branch_context_id)?
                .into_iter()
                .filter(|(_, composite)| composite.relative_file_path == fp)
                .filter_map(|(xref, _)| {
                    // Get the issue for this xref
                    issue_repo::get(conn, &xref.issue_id).ok()
                })
                .filter(|issue| seen.insert(issue.id.clone()))
                .map(|issue| ReviewIssueEntry {
                    id: issue.id,
                    comment: issue.comment,
                })
                .collect(),
        )
    } else {
        // Original implementation without file_path filter
        let eid_owned = eid.to_string();
        let bc_id_owned = branch_context_id.to_string();
        Ok(issue_repo::join_list(conn, |q| {
            q.filter(
                event_issue_composite_xref::event_id
                    .eq(eid_owned)
                    .and(event_issue_composite_xref::branch_context_id.eq(bc_id_owned)),
            )
        })?
        .into_iter()
        .filter(|(_, issue)| seen.insert(issue.id.clone()))
        .map(|(_, issue)| ReviewIssueEntry {
            id: issue.id,
            comment: issue.comment,
        })
        .collect())
    }
}
