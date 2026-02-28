use std::collections::{HashMap, HashSet};

use diesel::prelude::*;
use diesel::sqlite::SqliteConnection;

use crate::commands::event_commands::{CreateEventResponse, NewIssueInput};
use crate::db::schema::{branch_context, events};
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
use crate::utils::git::{diff_changed_files, run_git};

pub fn create_event(
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

    let created_issues = create_issues_with_reviews(conn, project_id, &new_event_id, new_issues)?;

    // Propagate unresolved xrefs from the previous event, translating line numbers for touched files
    let previous_event =
        find_previous_event(conn, project_id, &commit, &event_type, &new_event_id, path)?;

    if let Some(prev_event) = &previous_event {
        propagate_previous_xrefs(
            conn,
            PropagateXrefsParams {
                project_id,
                new_event_id: &new_event_id,
                prev_event_id: &prev_event.id,
                resolved_issues: &resolved_issues,
                project_path: path,
                commit: &commit,
                branch_context_id,
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
        run_git(project_path, &["log", "-1", "--format=%s", commit])
            .map(|o| o.stdout.trim().to_string())
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
    resolved_issues: &'a [String],
    project_path: &'a str,
    commit: &'a str,
    branch_context_id: &'a str,
}

fn propagate_previous_xrefs(
    conn: &mut SqliteConnection,
    params: PropagateXrefsParams,
) -> Result<(), AppError> {
    let resolved_set: HashSet<&str> = params.resolved_issues.iter().map(|s| s.as_str()).collect();

    let parent = format!("{}^", params.commit);
    let touched_file_paths: HashSet<String> =
        diff_changed_files(params.project_path, &[&parent, params.commit])
            .unwrap_or_default()
            .into_iter()
            .map(|f| f.path)
            .collect();

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

fn find_previous_event(
    conn: &mut SqliteConnection,
    project_id: &str,
    commit: &str,
    event_type: &str,
    new_event_id: &str,
    project_path: &str,
) -> Result<Option<crate::models::event::Event>, AppError> {
    if event_type == "resolution" {
        // Most recent existing event with the current commit hash (not the one we just created)
        let commit_owned = commit.to_string();
        let new_event_id_owned = new_event_id.to_string();
        let project_id_owned = project_id.to_string();
        let events = event_repo::list(conn, |q| {
            q.filter(
                events::project_id
                    .eq(project_id_owned)
                    .and(events::hash.eq(commit_owned))
                    .and(events::id.ne(new_event_id_owned)),
            )
            .order(events::created_at.desc())
        })?;
        Ok(events.into_iter().next())
    } else {
        // Most recent event with the parent commit hash
        let parent_hash = run_git(project_path, &["rev-parse", &format!("{}^", commit)])
            .map(|o| o.stdout.trim().to_string())
            .ok();

        if let Some(parent) = parent_hash {
            let project_id_owned = project_id.to_string();
            let events = event_repo::list(conn, |q| {
                q.filter(
                    events::project_id
                        .eq(project_id_owned)
                        .and(events::hash.eq(parent)),
                )
                .order(events::created_at.desc())
            })?;
            Ok(events.into_iter().next())
        } else {
            Ok(None)
        }
    }
}

fn translate_composite_line_numbers(
    prev_summary_metadata: &str,
    project_path: &str,
    current_commit: &str,
    file_path: &str,
) -> String {
    let parent = format!("{}^", current_commit);
    let diff_output = match run_git(
        project_path,
        &["diff", &parent, current_commit, "--", file_path],
    ) {
        Ok(o) => o.stdout,
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
