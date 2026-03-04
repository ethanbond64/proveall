use std::collections::{HashMap, HashSet};

use diesel::prelude::*;
use diesel::sqlite::SqliteConnection;

use crate::commands::event_commands::{CreateEventResponse, NewIssueInput};
use crate::db::schema::{branch_context, events, issues};
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

    // Find the previous event via head_event_id
    let branch_context = branch_context_repo::get(conn, branch_context_id)?;
    let previous_event = find_previous_event(
        conn,
        project_id,
        &commit,
        &event_type,
        // new_event_id not available yet — only used for resolution exclusion filter
        "",
        branch_context.head_event_id.as_deref(),
    )?;

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
        )?
    } else {
        previous_event.clone()
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

    // Propagate xrefs from the effective previous event to the target event
    // For resolution events, re-find previous excluding the newly created event
    let propagate_from = if event_type == "resolution" {
        find_previous_event(
            conn,
            project_id,
            &commit,
            &event_type,
            &new_event_id,
            branch_context.head_event_id.as_deref(),
        )?
    } else {
        effective_prev
    };

    if let Some(prev_event) = &propagate_from {
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
) -> Result<Option<crate::models::event::Event>, AppError> {
    let Some(prev_event) = previous_event else {
        return Ok(None);
    };

    let base_commit = match prev_event.hash.as_deref() {
        Some(h) => h,
        None => return Ok(Some(prev_event.clone())),
    };

    // Enumerate commits in the range (newest first)
    let range = format!("{}..{}", base_commit, target_commit);
    let log_output = run_git(project_path, &["log", "--format=%H", &range])
        .map(|o| o.stdout)
        .unwrap_or_default();

    let all_hashes: Vec<&str> = log_output.lines().filter(|l| !l.is_empty()).collect();

    // Remove the target commit (first in the list) and reverse to chronological order
    let intermediate_hashes: Vec<&str> = all_hashes
        .iter()
        .filter(|h| **h != target_commit)
        .copied()
        .rev()
        .collect();

    if intermediate_hashes.is_empty() {
        return Ok(Some(prev_event.clone()));
    }

    // Create intermediate events in chronological order, propagating xrefs through each
    let mut current_prev_event = prev_event.clone();

    for hash in intermediate_hashes {
        let summary = run_git(project_path, &["log", "-1", "--format=%s", hash])
            .map(|o| o.stdout.trim().to_string())
            .unwrap_or_default();

        let intermediate_event = event_repo::create(
            conn,
            NewEvent::new(
                project_id.to_string(),
                "commit".to_string(),
                Some(hash.to_string()),
                summary,
            ),
        )?;

        let prev_commit = current_prev_event
            .hash
            .as_deref()
            .unwrap_or(hash);

        propagate_previous_xrefs(
            conn,
            PropagateXrefsParams {
                project_id,
                new_event_id: &intermediate_event.id,
                prev_event_id: &current_prev_event.id,
                prev_commit,
                resolved_issues,
                project_path,
                commit: hash,
                branch_context_id,
            },
        )?;

        current_prev_event = intermediate_event;
    }

    Ok(Some(current_prev_event))
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
    prev_commit: &'a str,
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

    let touched_file_paths: HashSet<String> = if params.prev_commit == params.commit {
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

fn find_previous_event(
    conn: &mut SqliteConnection,
    project_id: &str,
    commit: &str,
    event_type: &str,
    new_event_id: &str,
    head_event_id: Option<&str>,
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
        // Use head_event_id from the branch context — this is the last reviewed event,
        // which works for both single-commit and bulk reviews.
        if let Some(heid) = head_event_id {
            Ok(Some(event_repo::get(conn, heid)?))
        } else {
            Ok(None)
        }
    }
}

fn translate_composite_line_numbers(
    prev_summary_metadata: &str,
    project_path: &str,
    prev_commit: &str,
    current_commit: &str,
    file_path: &str,
) -> String {
    let diff_output = match run_git(
        project_path,
        &["diff", prev_commit, current_commit, "--", file_path],
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::branch_context::NewBranchContext;
    use crate::models::project::NewProject;
    use diesel_migrations::MigrationHarness;
    use std::process::Command;
    use tempfile::TempDir;

    /// Create an in-memory SQLite connection with all migrations applied.
    fn test_conn() -> SqliteConnection {
        let mut conn = SqliteConnection::establish(":memory:")
            .expect("Failed to create in-memory connection");
        diesel::sql_query("PRAGMA foreign_keys = ON")
            .execute(&mut conn)
            .ok();
        conn.run_pending_migrations(crate::db::connection::MIGRATIONS)
            .expect("Failed to run migrations");
        conn
    }

    /// Create a temporary git repo with an initial commit and return the TempDir and base commit hash.
    fn setup_git_repo() -> (TempDir, String) {
        let dir = TempDir::new().expect("Failed to create temp dir");
        let path = dir.path();

        let git = |args: &[&str]| {
            Command::new("git")
                .args(args)
                .current_dir(path)
                .output()
                .expect("git command failed")
        };

        git(&["init"]);
        git(&["config", "user.email", "test@test.com"]);
        git(&["config", "user.name", "Test"]);

        // Create initial commit on main
        std::fs::write(path.join("file.txt"), "line1\n").unwrap();
        git(&["add", "."]);
        git(&["commit", "-m", "initial"]);

        // Get the initial commit hash
        let output = git(&["rev-parse", "HEAD"]);
        let base_hash = String::from_utf8_lossy(&output.stdout).trim().to_string();

        (dir, base_hash)
    }

    /// Add a commit to the repo and return its hash.
    fn add_commit(dir: &TempDir, filename: &str, content: &str, message: &str) -> String {
        let path = dir.path();
        std::fs::write(path.join(filename), content).unwrap();

        Command::new("git")
            .args(["add", "."])
            .current_dir(path)
            .output()
            .unwrap();
        Command::new("git")
            .args(["commit", "-m", message])
            .current_dir(path)
            .output()
            .unwrap();

        let output = Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(path)
            .output()
            .unwrap();
        String::from_utf8_lossy(&output.stdout).trim().to_string()
    }

    /// Set up project and branch context in the DB. Returns (project_id, branch_context_id).
    fn setup_db_records(conn: &mut SqliteConnection, repo_path: &str) -> (String, String) {
        let project = project_repo::create(
            conn,
            NewProject::new(repo_path.to_string(), Some("test".to_string())),
        )
        .expect("Failed to create project");

        let bc = branch_context_repo::create(
            conn,
            NewBranchContext::new(
                project.id.clone(),
                "main".to_string(),
                "main".to_string(),
                "{}".to_string(),
            ),
        )
        .expect("Failed to create branch context");

        (project.id, bc.id)
    }

    #[test]
    fn test_single_commit_creates_no_intermediates() {
        let mut conn = test_conn();
        let (dir, base_hash) = setup_git_repo();
        let repo_path = dir.path().to_str().unwrap();
        let (project_id, bc_id) = setup_db_records(&mut conn, repo_path);

        // Create a single commit after the base
        let c1 = add_commit(&dir, "file.txt", "line1\nline2\n", "commit 1");

        // Review the base commit first to set up head_event_id
        let base_result = create_event(
            &mut conn,
            &project_id,
            base_hash.clone(),
            "commit".to_string(),
            vec![],
            vec![],
            &bc_id,
        )
        .expect("Failed to create base event");

        // Now review c1 — should be a single-commit review (no intermediates)
        let result = create_event(
            &mut conn,
            &project_id,
            c1.clone(),
            "commit".to_string(),
            vec![],
            vec![],
            &bc_id,
        )
        .expect("Failed to create event for c1");

        // Should have exactly 2 events total: base + c1
        let all_events = event_repo::list(&mut conn, |q| q).expect("Failed to list events");
        assert_eq!(all_events.len(), 2, "Expected 2 events (base + c1)");

        // head_event_id should point to the c1 event
        let bc = branch_context_repo::get(&mut conn, &bc_id).unwrap();
        assert_eq!(bc.head_event_id, Some(result.id.clone()));

        // The c1 event should have the correct hash
        let c1_event = event_repo::get(&mut conn, &result.id).unwrap();
        assert_eq!(c1_event.hash, Some(c1));
    }

    #[test]
    fn test_bulk_creates_intermediate_events() {
        let mut conn = test_conn();
        let (dir, base_hash) = setup_git_repo();
        let repo_path = dir.path().to_str().unwrap();
        let (project_id, bc_id) = setup_db_records(&mut conn, repo_path);

        // Create 3 commits after base
        let c1 = add_commit(&dir, "file.txt", "line1\nline2\n", "commit 1");
        let c2 = add_commit(&dir, "file.txt", "line1\nline2\nline3\n", "commit 2");
        let c3 = add_commit(&dir, "file.txt", "line1\nline2\nline3\nline4\n", "commit 3");

        // Review the base commit to set up head_event_id
        let _base_result = create_event(
            &mut conn,
            &project_id,
            base_hash.clone(),
            "commit".to_string(),
            vec![],
            vec![],
            &bc_id,
        )
        .expect("Failed to create base event");

        // Now do a bulk review targeting c3 (skipping c1 and c2)
        let result = create_event(
            &mut conn,
            &project_id,
            c3.clone(),
            "commit".to_string(),
            vec![],
            vec![],
            &bc_id,
        )
        .expect("Failed to create bulk event");

        // Should have 4 events total: base + c1 (intermediate) + c2 (intermediate) + c3 (target)
        let all_events = event_repo::list(&mut conn, |q| q).expect("Failed to list events");
        assert_eq!(
            all_events.len(),
            4,
            "Expected 4 events (base + 2 intermediates + target)"
        );

        // Verify each intermediate commit has an event with the correct hash
        let c1_events: Vec<_> = all_events
            .iter()
            .filter(|e| e.hash.as_deref() == Some(&c1))
            .collect();
        assert_eq!(c1_events.len(), 1, "Expected exactly 1 event for c1");

        let c2_events: Vec<_> = all_events
            .iter()
            .filter(|e| e.hash.as_deref() == Some(&c2))
            .collect();
        assert_eq!(c2_events.len(), 1, "Expected exactly 1 event for c2");

        // head_event_id should point to the target (c3) event
        let bc = branch_context_repo::get(&mut conn, &bc_id).unwrap();
        assert_eq!(bc.head_event_id, Some(result.id.clone()));

        // The target event should have the c3 hash
        let target_event = event_repo::get(&mut conn, &result.id).unwrap();
        assert_eq!(target_event.hash, Some(c3));
    }

    #[test]
    fn test_bulk_propagates_xrefs_through_intermediates() {
        let mut conn = test_conn();
        let (dir, base_hash) = setup_git_repo();
        let repo_path = dir.path().to_str().unwrap();
        let (project_id, bc_id) = setup_db_records(&mut conn, repo_path);

        // Create 3 commits
        let c1 = add_commit(&dir, "file.txt", "line1\nline2\n", "commit 1");
        let c2 = add_commit(&dir, "file.txt", "line1\nline2\nline3\n", "commit 2");
        let c3 = add_commit(&dir, "file.txt", "line1\nline2\nline3\nline4\n", "commit 3");

        // Review base commit
        let _base_result = create_event(
            &mut conn,
            &project_id,
            base_hash.clone(),
            "commit".to_string(),
            vec![],
            vec![],
            &bc_id,
        )
        .expect("Failed to create base event");

        // Review c1 with an issue (creates xrefs that should propagate)
        use crate::commands::event_commands::{NewIssueInput, NewIssueReviewInput};
        let c1_result = create_event(
            &mut conn,
            &project_id,
            c1.clone(),
            "commit".to_string(),
            vec![NewIssueInput {
                comment: "test issue".to_string(),
                reviews: vec![NewIssueReviewInput {
                    type_: "line".to_string(),
                    path: Some("file.txt".to_string()),
                    start: Some(1),
                    end: Some(2),
                    state: "red".to_string(),
                }],
            }],
            vec![],
            &bc_id,
        )
        .expect("Failed to create c1 event with issue");

        // Verify xrefs exist on c1's event
        let c1_xrefs = event_issue_composite_xref_repo::join_list_by_event(
            &mut conn,
            &c1_result.id,
            &bc_id,
        )
        .expect("Failed to get c1 xrefs");
        assert_eq!(c1_xrefs.len(), 1, "Expected 1 xref on c1 event");

        // Now bulk review c3 (skipping c2)
        let c3_result = create_event(
            &mut conn,
            &project_id,
            c3.clone(),
            "commit".to_string(),
            vec![],
            vec![],
            &bc_id,
        )
        .expect("Failed to create bulk event for c3");

        // Verify c2 has an intermediate event
        let all_events = event_repo::list(&mut conn, |q| q).expect("Failed to list events");
        let c2_event = all_events
            .iter()
            .find(|e| e.hash.as_deref() == Some(c2.as_str()))
            .expect("Expected an event for c2");

        // c2's intermediate event should have propagated xrefs from c1
        let c2_xrefs = event_issue_composite_xref_repo::join_list_by_event(
            &mut conn,
            &c2_event.id,
            &bc_id,
        )
        .expect("Failed to get c2 xrefs");
        assert_eq!(
            c2_xrefs.len(),
            1,
            "Expected 1 xref propagated to c2 intermediate"
        );

        // c3's target event should also have propagated xrefs
        let c3_xrefs =
            event_issue_composite_xref_repo::join_list_by_event(&mut conn, &c3_result.id, &bc_id)
                .expect("Failed to get c3 xrefs");
        assert_eq!(
            c3_xrefs.len(),
            1,
            "Expected 1 xref propagated to c3 target"
        );

        // All xrefs should reference the same issue
        let issue_id = &c1_xrefs[0].0.issue_id;
        assert_eq!(&c2_xrefs[0].0.issue_id, issue_id);
        assert_eq!(&c3_xrefs[0].0.issue_id, issue_id);
    }

    #[test]
    fn test_bulk_issues_attached_to_target_only() {
        let mut conn = test_conn();
        let (dir, base_hash) = setup_git_repo();
        let repo_path = dir.path().to_str().unwrap();
        let (project_id, bc_id) = setup_db_records(&mut conn, repo_path);

        // Create 2 commits after base
        let c1 = add_commit(&dir, "file.txt", "line1\nline2\n", "commit 1");
        let c2 = add_commit(&dir, "file.txt", "line1\nline2\nline3\n", "commit 2");

        // Review base
        let _base_result = create_event(
            &mut conn,
            &project_id,
            base_hash.clone(),
            "commit".to_string(),
            vec![],
            vec![],
            &bc_id,
        )
        .expect("Failed to create base event");

        // Bulk review c2 with a new issue
        use crate::commands::event_commands::{NewIssueInput, NewIssueReviewInput};
        let c2_result = create_event(
            &mut conn,
            &project_id,
            c2.clone(),
            "commit".to_string(),
            vec![NewIssueInput {
                comment: "bulk issue".to_string(),
                reviews: vec![NewIssueReviewInput {
                    type_: "line".to_string(),
                    path: Some("file.txt".to_string()),
                    start: Some(1),
                    end: Some(3),
                    state: "yellow".to_string(),
                }],
            }],
            vec![],
            &bc_id,
        )
        .expect("Failed to create bulk event");

        // Get the intermediate event for c1
        let all_events = event_repo::list(&mut conn, |q| q).expect("Failed to list events");
        let c1_event = all_events
            .iter()
            .find(|e| e.hash.as_deref() == Some(c1.as_str()))
            .expect("Expected an event for c1");

        // The issue should be created with created_event_id pointing to the target (c2) event
        let all_issues = issue_repo::list(&mut conn, |q| q).expect("Failed to list issues");
        assert_eq!(all_issues.len(), 1, "Expected exactly 1 issue");
        assert_eq!(
            all_issues[0].created_event_id, c2_result.id,
            "Issue should be attached to the target event, not an intermediate"
        );

        // c1 intermediate should have no xrefs (no prior issues to propagate, no new issues)
        let c1_xrefs = event_issue_composite_xref_repo::join_list_by_event(
            &mut conn,
            &c1_event.id,
            &bc_id,
        )
        .expect("Failed to get c1 xrefs");
        assert_eq!(
            c1_xrefs.len(),
            0,
            "Intermediate c1 should have no xrefs (no prior issues)"
        );

        // c2 target should have xrefs for the new issue
        let c2_xrefs =
            event_issue_composite_xref_repo::join_list_by_event(&mut conn, &c2_result.id, &bc_id)
                .expect("Failed to get c2 xrefs");
        assert_eq!(c2_xrefs.len(), 1, "Target c2 should have 1 xref");
    }

    #[test]
    fn test_no_previous_event_no_intermediates() {
        let mut conn = test_conn();
        let (dir, base_hash) = setup_git_repo();
        let repo_path = dir.path().to_str().unwrap();
        let (project_id, bc_id) = setup_db_records(&mut conn, repo_path);

        // Create the first event ever (no head_event_id set yet)
        let result = create_event(
            &mut conn,
            &project_id,
            base_hash.clone(),
            "commit".to_string(),
            vec![],
            vec![],
            &bc_id,
        )
        .expect("Failed to create first event");

        // Should have exactly 1 event
        let all_events = event_repo::list(&mut conn, |q| q).expect("Failed to list events");
        assert_eq!(all_events.len(), 1, "Expected 1 event");

        // head_event_id should be set
        let bc = branch_context_repo::get(&mut conn, &bc_id).unwrap();
        assert_eq!(bc.head_event_id, Some(result.id));
    }
}
