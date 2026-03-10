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
use crate::utils::git::{self, diff_changed_files};

#[cfg(test)]
mod tests;

enum ReviewType {
    Commit,
    Branch,
    MergeReview,
}

impl ReviewType {
    fn parse(s: &str) -> Result<Self, AppError> {
        match s {
            "commit" => Ok(ReviewType::Commit),
            "branch" => Ok(ReviewType::Branch),
            "merge_review" => Ok(ReviewType::MergeReview),
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
        ReviewType::Commit => {
            let base_ref = resolve_commit_base_ref(
                conn,
                branch_context.head_event_id.as_deref(),
                &branch_context.base_branch,
            )?;
            (
                build_commit_touched_files(path, &commit, &base_ref, &branch_context.base_branch)?,
                build_issues_from_event(
                    conn,
                    branch_context.head_event_id.as_deref(),
                    None,
                    branch_context_id,
                )?,
            )
        }
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
        ReviewType::MergeReview => (
            build_merge_review_touched_files(path, &commit)?,
            build_issues_from_event(
                conn,
                branch_context.head_event_id.as_deref(),
                None,
                branch_context_id,
            )?,
        ),
    };

    Ok(ReviewFileSystemDataResponse {
        touched_files,
        issues,
    })
}

/// Resolve the base ref for a commit-mode diff.
/// If `head_event_id` exists, use that event's commit hash.
/// Otherwise fall back to the base branch.
fn resolve_commit_base_ref(
    conn: &mut SqliteConnection,
    head_event_id: Option<&str>,
    base_branch: &str,
) -> Result<String, AppError> {
    if let Some(eid) = head_event_id {
        let prev_event = event_repo::get(conn, eid)?;
        if let Some(hash) = prev_event.hash {
            return Ok(hash);
        }
    }
    Ok(base_branch.to_string())
}

fn build_commit_touched_files(
    project_path: &str,
    commit: &str,
    base_ref: &str,
    base_branch: &str,
) -> Result<Vec<TouchedFile>, AppError> {
    let diff_files = diff_changed_files(project_path, &[base_ref, commit])?;

    // Enumerate first-parent commits in the range to find base-branch merges.
    // Only files exclusively introduced by base-branch merges (and not conflict
    // resolutions) are marked merge_only. Everything else stays normal.
    let range = format!("{}..{}", base_ref, commit);
    let commits = git::log(project_path, &["--first-parent", &range]).unwrap_or_default();

    // Collect files that were introduced only via base-branch merges.
    // A file is merge_only if it appears in a base-merge diff but NOT in any
    // non-merge commit diff and NOT as a conflict resolution file.
    let mut merge_introduced_files: HashSet<String> = HashSet::new();
    let mut non_merge_files: HashSet<String> = HashSet::new();

    for c in &commits {
        let is_merge = c.parents.len() > 1;
        let is_base_merge = is_merge
            && git::is_base_branch_merge(project_path, &c.parents, base_branch);

        if is_base_merge {
            // Conflict resolution files are NOT merge_only — add to non_merge_files
            if let Ok(cc_output) = git::diff_tree_cc_name_only(project_path, &c.hash) {
                for line in cc_output.lines().filter(|l| !l.is_empty()) {
                    non_merge_files.insert(line.to_string());
                }
            }

            // All files changed by this merge (diff first-parent vs merge commit)
            if let Some(parent) = c.parents.first() {
                if let Ok(files) = diff_changed_files(project_path, &[parent, &c.hash]) {
                    for f in files {
                        merge_introduced_files.insert(f.path);
                    }
                }
            }
        } else {
            // Non-merge commit: all its files are non-merge
            if let Some(parent) = c.parents.first() {
                if let Ok(files) = diff_changed_files(project_path, &[parent, &c.hash]) {
                    for f in files {
                        non_merge_files.insert(f.path);
                    }
                }
            }
        }
    }

    Ok(diff_files
        .into_iter()
        .map(|f| {
            let merge_only = merge_introduced_files.contains(&f.path)
                && !non_merge_files.contains(&f.path);
            TouchedFile {
                name: extract_file_name(&f.path),
                path: f.path,
                diff_mode: Some(f.status),
                state: "red".to_string(),
                merge_only,
            }
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
                merge_only: false,
            }
        })
        .collect())
}

/// Build touched files for merge review.
/// Conflict resolution files (from `git diff-tree --cc`) are included as normal files.
/// All other files introduced by the merge (diff against first parent) are included
/// as `merge_only: true` so the frontend can show them in a separate section.
fn build_merge_review_touched_files(
    project_path: &str,
    commit: &str,
) -> Result<Vec<TouchedFile>, AppError> {
    let cc_output = git::diff_tree_cc_name_only(project_path, commit)?;
    let conflict_files: HashSet<String> = cc_output
        .lines()
        .filter(|l| !l.is_empty())
        .map(|s| s.to_string())
        .collect();

    let mut touched_files: Vec<TouchedFile> = conflict_files
        .iter()
        .map(|file_path| TouchedFile {
            name: extract_file_name(file_path),
            path: file_path.clone(),
            diff_mode: Some("M".to_string()),
            state: "red".to_string(),
            merge_only: false,
        })
        .collect();

    // Also include non-conflict files from the merge (diff first parent vs merge commit)
    let first_parent = format!("{}^1", commit);
    if let Ok(all_merge_files) = diff_changed_files(project_path, &[&first_parent, commit]) {
        for f in all_merge_files {
            if !conflict_files.contains(&f.path) {
                touched_files.push(TouchedFile {
                    name: extract_file_name(&f.path),
                    path: f.path,
                    diff_mode: Some(f.status),
                    state: "red".to_string(),
                    merge_only: true,
                });
            }
        }
    }

    Ok(touched_files)
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
    let content = git::show(path, &ref_path).unwrap_or_default();

    let (diff, line_summary, issues) = match review_type {
        ReviewType::Commit => {
            let base_ref = resolve_commit_base_ref(
                conn,
                branch_context.head_event_id.as_deref(),
                &branch_context.base_branch,
            )?;
            let base_file_ref = format!("{}:{}", base_ref, file_path);
            let diff = Some(git::show(path, &base_file_ref).unwrap_or_default());
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
            let diff = Some(git::show(path, &base_ref).unwrap_or_default());
            let line_summary =
                build_branch_line_summary(conn, event_id, &file_path, branch_context_id, issue_id)?;
            let issues =
                build_issues_from_event(conn, event_id, Some(&file_path), branch_context_id)?;
            (diff, line_summary, issues)
        }
        ReviewType::MergeReview => {
            // For merge review, show the first parent's version as the "before" (feature branch
            // before the merge) so the user sees only what the conflict resolution changed.
            let first_parent_ref = format!("{}^1:{}", commit, file_path);
            let diff = Some(git::show(path, &first_parent_ref).unwrap_or_default());
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
