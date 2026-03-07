use std::collections::{HashMap, HashSet};

use diesel::prelude::*;
use diesel::sqlite::SqliteConnection;
use diesel::Connection;

use crate::commands::event_commands::{CreateEventResponse, NewIssueInput};
use crate::db::schema::{branch_context, issues};
use crate::error::AppError;
use crate::models::composite_file_review_state::{
    NewCompositeFileReviewState, ReviewSummaryMetadataEntry,
};
use crate::models::event::NewEvent;
use crate::models::event_issue_composite_xref::EventIssueCompositeXref;
use crate::models::issue::NewIssue;
use crate::models::review::NewReview;
use crate::repositories::{
    branch_context_repo, composite_file_review_state_repo, event_issue_composite_xref_repo,
    event_repo, issue_repo, project_repo, review_repo,
};
use crate::utils::git::{self, diff_changed_files};

pub fn create_event(
    conn: &mut SqliteConnection,
    project_id: &str,
    commit: String,
    event_type: String,
    new_issues: Vec<NewIssueInput>,
    resolved_issues: Vec<String>,
    branch_context_id: &str,
) -> Result<CreateEventResponse, AppError> {
    conn.transaction(|conn| {
        create_event_inner(
            conn,
            project_id,
            commit,
            event_type,
            new_issues,
            resolved_issues,
            branch_context_id,
        )
    })
}

fn create_event_inner(
    conn: &mut SqliteConnection,
    project_id: &str,
    commit: String,
    event_type: String,
    new_issues: Vec<NewIssueInput>,
    resolved_issues: Vec<String>,
    branch_context_id: &str,
) -> Result<CreateEventResponse, AppError> {
    let project = project_repo::get(conn, project_id)?;
    let path = &project.path;

    // Validation — resolution events cannot have new issues and must have resolved issues
    if event_type == "resolution" {
        if !new_issues.is_empty() {
            return Err(AppError::Git(
                "Resolution events cannot have new issues".to_string(),
            ));
        }
        if resolved_issues.is_empty() {
            return Err(AppError::Git(
                "Resolution events must have resolved issues".to_string(),
            ));
        }
    }

    let branch_context = branch_context_repo::get(conn, branch_context_id)?;

    let previous_event = match branch_context.head_event_id.as_deref() {
        Some(heid) => Some(event_repo::get(conn, heid)?),
        None => None,
    };

    // For commit events: create intermediate events for bulk reviews before the target event
    let effective_prev = if event_type == "commit" {
        create_intermediate_events(
            conn,
            project_id,
            &commit,
            previous_event.as_ref(),
            &resolved_issues,
            path,
            branch_context_id,
            &branch_context.base_branch,
        )?
    } else {
        previous_event
    };

    // Create the target event
    let summary = build_event_summary(conn, path, &commit, &event_type, &resolved_issues);
    let event = event_repo::create(
        conn,
        NewEvent::new(
            project_id.to_string(),
            event_type.clone(),
            Some(commit.clone()),
            summary,
        ),
    )?;
    let new_event_id = event.id.clone();

    // Create issues with reviews — attached to the target event
    let created_issues = create_issues_with_reviews(conn, project_id, &new_event_id, new_issues)?;

    // Mark resolved issues with the resolution event ID
    for issue_id in &resolved_issues {
        issue_repo::update(
            conn,
            issue_id,
            issues::resolved_event_id.eq(Some(&new_event_id)),
        )?;
    }

    if let Some(prev_event) = &effective_prev {
        let prev_commit = prev_event.hash.as_deref().unwrap_or(&commit);
        propagate_previous_xrefs(
            conn,
            PropagateXrefsParams {
                project_id,
                new_event_id: &new_event_id,
                prev_event_id: &prev_event.id,
                prev_commit,
                resolved_issues: &resolved_issues,
                project_path: path,
                commit: &commit,
                branch_context_id,
                skip_file_translation: false,
            },
        )?;
    }

    // Create composites + xrefs for new issues' non-green reviews
    create_composites_for_new_issues(
        conn,
        project_id,
        &new_event_id,
        &created_issues,
        branch_context_id,
    )?;

    // Update branch_context HEAD to point to this new event
    branch_context_repo::update(
        conn,
        branch_context_id,
        branch_context::head_event_id.eq(&new_event_id),
    )?;

    Ok(CreateEventResponse { id: new_event_id })
}

/// Create intermediate events for commits between the previous event's commit and the target commit.
/// Returns the effective previous event — either the last intermediate created, or the original
/// previous event if no intermediates were needed.
fn create_intermediate_events(
    conn: &mut SqliteConnection,
    project_id: &str,
    target_commit: &str,
    previous_event: Option<&crate::models::event::Event>,
    resolved_issues: &[String],
    project_path: &str,
    branch_context_id: &str,
    base_branch: &str,
) -> Result<Option<crate::models::event::Event>, AppError> {
    let branch_context = branch_context_repo::get(conn, branch_context_id)?;

    let base_commit_owned: Option<String> = match previous_event {
        Some(prev) => prev.hash.clone(),
        None => {
            // First review on this branch — use the HEAD of the base branch as the starting point
            git::rev_parse(project_path, &branch_context.base_branch).ok()
        }
    };

    let Some(base_commit) = base_commit_owned.as_deref() else {
        return Ok(previous_event.cloned());
    };

    // Enumerate commits in the range (newest first)
    let range = format!("{}..{}", base_commit, target_commit);
    let all_commits = git::log(project_path, &[&range]).unwrap_or_default();

    // Remove the target commit (first in the list) and reverse to chronological order
    let intermediate_commits: Vec<_> = all_commits
        .iter()
        .filter(|c| c.hash != target_commit)
        .rev()
        .collect();

    if intermediate_commits.is_empty() {
        return Ok(previous_event.cloned());
    }

    // Create intermediate events in chronological order, propagating xrefs through each
    let mut current_prev_event: Option<crate::models::event::Event> = previous_event.cloned();

    for commit in intermediate_commits {
        let intermediate_event = event_repo::create(
            conn,
            NewEvent::new(
                project_id.to_string(),
                "commit".to_string(),
                Some(commit.hash.clone()),
                commit.subject.clone(),
            ),
        )?;

        // Only propagate xrefs if there is a previous event to propagate from
        if let Some(ref prev) = current_prev_event {
            let prev_commit = prev.hash.as_deref().unwrap_or(&commit.hash);

            // Base-branch merges skip file translation — their file changes are from
            // the base branch, not feature work
            let is_base_merge = commit.parents.len() > 1
                && git::is_base_branch_merge(project_path, &commit.parents, base_branch);

            propagate_previous_xrefs(
                conn,
                PropagateXrefsParams {
                    project_id,
                    new_event_id: &intermediate_event.id,
                    prev_event_id: &prev.id,
                    prev_commit,
                    resolved_issues,
                    project_path,
                    commit: &commit.hash,
                    branch_context_id,
                    skip_file_translation: is_base_merge,
                },
            )?;
        }

        current_prev_event = Some(intermediate_event);
    }

    Ok(current_prev_event)
}

fn build_event_summary(
    conn: &mut SqliteConnection,
    project_path: &str,
    commit: &str,
    event_type: &str,
    resolved_issues: &[String],
) -> String {
    if event_type == "resolution" {
        let comments: Vec<String> = resolved_issues
            .iter()
            .filter_map(|iid| issue_repo::get(conn, iid).ok())
            .map(|issue| issue.comment)
            .collect();
        serde_json::to_string(&comments).unwrap_or_default()
    } else {
        git::log(project_path, &["-1", commit])
            .ok()
            .and_then(|v| v.into_iter().next())
            .map(|c| c.subject)
            .unwrap_or_default()
    }
}

fn create_issues_with_reviews(
    conn: &mut SqliteConnection,
    project_id: &str,
    event_id: &str,
    new_issues: Vec<NewIssueInput>,
) -> Result<Vec<(String, NewIssueInput)>, AppError> {
    let mut created = Vec::new();
    for new_issue in new_issues {
        let issue = issue_repo::create(
            conn,
            NewIssue::new(
                project_id.to_string(),
                event_id.to_string(),
                new_issue.comment.clone(),
            ),
        )?;

        for review in &new_issue.reviews {
            review_repo::create(
                conn,
                NewReview::new(
                    issue.id.clone(),
                    review.type_.clone(),
                    review.path.clone(),
                    review.start,
                    review.end,
                    "state".to_string(),
                    review.state.clone(),
                ),
            )?;
        }

        created.push((issue.id, new_issue));
    }
    Ok(created)
}

fn create_composite_with_xref(
    conn: &mut SqliteConnection,
    project_id: &str,
    event_id: &str,
    issue_id: &str,
    file_path: String,
    summary_metadata: String,
    branch_context_id: &str,
) -> Result<(), AppError> {
    let composite = composite_file_review_state_repo::create(
        conn,
        NewCompositeFileReviewState::new(project_id.to_string(), file_path, summary_metadata),
    )?;
    event_issue_composite_xref_repo::create(
        conn,
        EventIssueCompositeXref {
            event_id: event_id.to_string(),
            issue_id: issue_id.to_string(),
            composite_file_id: composite.id,
            branch_context_id: branch_context_id.to_string(),
        },
    )?;
    Ok(())
}

struct PropagateXrefsParams<'a> {
    project_id: &'a str,
    new_event_id: &'a str,
    prev_event_id: &'a str,
    prev_commit: &'a str,
    resolved_issues: &'a [String],
    project_path: &'a str,
    commit: &'a str,
    branch_context_id: &'a str,
    skip_file_translation: bool,
}

fn propagate_previous_xrefs(
    conn: &mut SqliteConnection,
    params: PropagateXrefsParams,
) -> Result<(), AppError> {
    let resolved_set: HashSet<&str> = params.resolved_issues.iter().map(|s| s.as_str()).collect();

    let touched_file_paths: HashSet<String> =
        if params.skip_file_translation || params.prev_commit == params.commit {
            // skip_file_translation: base-branch merge — reuse composites without line translation
            // Same commit (e.g. resolution events) — no file changes to translate
            HashSet::new()
        } else {
            diff_changed_files(params.project_path, &[params.prev_commit, params.commit])
                .unwrap_or_default()
                .into_iter()
                .map(|f| f.path)
                .collect()
        };

    let prev_xrefs_with_composites = event_issue_composite_xref_repo::join_list_by_event(
        conn,
        params.prev_event_id,
        params.branch_context_id,
    )?;

    for (prev_xref, prev_composite) in prev_xrefs_with_composites {
        if resolved_set.contains(prev_xref.issue_id.as_str()) {
            continue;
        }

        if touched_file_paths.contains(&prev_composite.relative_file_path) {
            // File was touched — translate line numbers and create new composite
            let translated_metadata = translate_composite_line_numbers(
                &prev_composite.summary_metadata,
                params.project_path,
                params.prev_commit,
                params.commit,
                &prev_composite.relative_file_path,
            );
            create_composite_with_xref(
                conn,
                params.project_id,
                params.new_event_id,
                &prev_xref.issue_id,
                prev_composite.relative_file_path.clone(),
                translated_metadata,
                params.branch_context_id,
            )?;
        } else {
            // File not touched — reuse existing composite
            event_issue_composite_xref_repo::create(
                conn,
                EventIssueCompositeXref {
                    event_id: params.new_event_id.to_string(),
                    issue_id: prev_xref.issue_id,
                    composite_file_id: prev_xref.composite_file_id,
                    branch_context_id: params.branch_context_id.to_string(),
                },
            )?;
        }
    }

    Ok(())
}

fn create_composites_for_new_issues(
    conn: &mut SqliteConnection,
    project_id: &str,
    event_id: &str,
    created_issues: &[(String, NewIssueInput)],
    branch_context_id: &str,
) -> Result<(), AppError> {
    for (issue_id, new_issue) in created_issues {
        // Pre-group non-green reviews by file path so each file's composite is built in one pass
        let mut entries_by_file: HashMap<String, Vec<ReviewSummaryMetadataEntry>> = HashMap::new();
        for review in &new_issue.reviews {
            if review.state == "green" {
                continue;
            }
            if let Some(path) = &review.path {
                entries_by_file
                    .entry(path.clone())
                    .or_default()
                    .push(ReviewSummaryMetadataEntry {
                        start: review.start.unwrap_or(0),
                        end: review.end.unwrap_or(0),
                        state: review.state.clone(),
                    });
            }
        }

        for (file_path, summary_entries) in entries_by_file {
            let summary_metadata = serde_json::to_string(&summary_entries).unwrap_or_default();
            create_composite_with_xref(
                conn,
                project_id,
                event_id,
                issue_id,
                file_path,
                summary_metadata,
                branch_context_id,
            )?;
        }
    }

    Ok(())
}

fn translate_composite_line_numbers(
    prev_summary_metadata: &str,
    project_path: &str,
    prev_commit: &str,
    current_commit: &str,
    file_path: &str,
) -> String {
    let diff_output = match git::diff_file(project_path, prev_commit, current_commit, file_path) {
        Ok(o) => o,
        Err(_) => return prev_summary_metadata.to_string(),
    };

    let hunks = parse_diff_hunks(&diff_output);
    if hunks.is_empty() {
        return prev_summary_metadata.to_string();
    }

    let entries: Vec<ReviewSummaryMetadataEntry> = match serde_json::from_str(prev_summary_metadata)
    {
        Ok(e) => e,
        Err(_) => return prev_summary_metadata.to_string(),
    };

    let translated: Vec<ReviewSummaryMetadataEntry> = entries
        .into_iter()
        .map(|e| {
            let new_start = translate_line(e.start, &hunks);
            let new_end = translate_line(e.end, &hunks);
            ReviewSummaryMetadataEntry {
                start: new_start,
                end: new_end.max(new_start),
                state: e.state,
            }
        })
        .collect();

    serde_json::to_string(&translated).unwrap_or_else(|_| prev_summary_metadata.to_string())
}

// ---------------------------------------------------------------------------
// Diff hunk parsing & line translation
// ---------------------------------------------------------------------------

struct DiffHunk {
    old_start: i32,
    old_count: i32,
    new_start: i32,
    new_count: i32,
}

/// Parse unified diff output and extract hunk headers.
fn parse_diff_hunks(diff_output: &str) -> Vec<DiffHunk> {
    diff_output
        .lines()
        .filter(|l| l.starts_with("@@"))
        .filter_map(parse_hunk_header)
        .collect()
}

/// Parse a single hunk header line: `@@ -A,B +C,D @@`
/// Count defaults to 1 when omitted (e.g. `@@ -5 +5,3 @@`).
fn parse_hunk_header(line: &str) -> Option<DiffHunk> {
    // Strip leading "@@" and find the closing "@@"
    let inner = line.strip_prefix("@@")?.split("@@").next()?.trim();
    let mut parts = inner.split_whitespace();

    let old_part = parts.next()?.strip_prefix('-')?;
    let new_part = parts.next()?.strip_prefix('+')?;

    let (old_start, old_count) = parse_hunk_range(old_part)?;
    let (new_start, new_count) = parse_hunk_range(new_part)?;

    Some(DiffHunk {
        old_start,
        old_count,
        new_start,
        new_count,
    })
}

fn parse_hunk_range(s: &str) -> Option<(i32, i32)> {
    if let Some((start, count)) = s.split_once(',') {
        Some((start.parse().ok()?, count.parse().ok()?))
    } else {
        Some((s.parse().ok()?, 1))
    }
}

/// Translate a single line number from the old file to the new file
/// using the ordered list of diff hunks.
fn translate_line(old_line: i32, hunks: &[DiffHunk]) -> i32 {
    let mut cumulative_offset: i32 = 0;

    for hunk in hunks {
        let old_end = hunk.old_start + hunk.old_count; // exclusive

        if old_line < hunk.old_start {
            return old_line + cumulative_offset;
        }

        if old_line < old_end {
            // Line falls inside a modified region
            if hunk.new_count == 0 {
                // Pure deletion — map to where the deletion happened
                return hunk.new_start;
            }
            // Proportional mapping within the hunk
            let ratio = (old_line - hunk.old_start) as f64 / hunk.old_count.max(1) as f64;
            let mapped = hunk.new_start + (ratio * hunk.new_count as f64) as i32;
            return mapped.min(hunk.new_start + hunk.new_count - 1);
        }

        cumulative_offset += hunk.new_count - hunk.old_count;
    }

    // After all hunks
    old_line + cumulative_offset
}

#[cfg(test)]
mod tests;
