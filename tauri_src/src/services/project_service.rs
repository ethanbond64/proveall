use std::collections::{HashMap, HashSet};

use diesel::prelude::*;
use diesel::sqlite::SqliteConnection;

use crate::commands::project_commands::{
    BranchData, DiffSummary, EventEntry, IssueEntry, ProjectStateResponse,
};
use crate::db::schema::{branch_context, event_issue_composite_xref, events};
use crate::error::AppError;
use crate::models::event::NewEvent;
use crate::repositories::{branch_context_repo, event_repo, issue_repo, project_repo};
use crate::services::event_service::{propagate_previous_xrefs, PropagateXrefsParams};
use crate::utils::git::{self, diff_changed_files, parse_stat};

#[cfg(test)]
mod tests;

/// Build the full project state for a branch context.
///
/// **Side effects:** This read path lazily creates events for base-branch merge commits.
/// When all commits older than a base-branch merge already have events, this function
/// auto-creates an event for the merge, propagates xrefs (with skip_file_translation
/// so only feature-branch file changes are tracked), and advances the branch_context
/// head_event_id. This keeps the xref chain intact without requiring user interaction
/// for base-branch merges.
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

    // TODO this range is NOT the true range of commits that you see in github when you go to merge into the base branch - instead it is the diff between main HEAD and now...
    let range = format!("{}..HEAD", base_branch);
    let diff_summary = build_diff_summary(path, &range)?;
    let mut event_entries = build_event_entries(conn, path, &range, project_id, &base_branch)?;

    // Side effect: lazily create events for base-branch merge commits whose
    // predecessors all have events already. This maintains the xref chain.
    lazy_create_base_merge_events(
        conn,
        &mut event_entries,
        project_id,
        path,
        branch_context_id,
    )?;

    // Re-read branch_context since lazy creation may have updated head_event_id.
    let bc = branch_context_repo::get(conn, branch_context_id)?;
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
    base_branch: &str,
) -> Result<Vec<EventEntry>, AppError> {
    let commits = git::log(path, &[range]).unwrap_or_default();

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
            let is_merge = commit.parents.len() > 1;
            let is_base_merge =
                is_merge && git::is_base_branch_merge(path, &commit.parents, base_branch);
            let has_conflict_changes = is_base_merge
                && git::diff_tree_cc(path, &commit.hash)
                    .map(|output| !output.trim().is_empty())
                    .unwrap_or(false);

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
                        is_base_merge,
                        has_conflict_changes,
                    })
                    .collect::<Vec<_>>()
            } else {
                vec![EventEntry {
                    id: None,
                    event_type: "commit".to_string(),
                    commit: commit.hash.clone(),
                    author: Some(commit.author.clone()),
                    message: commit.subject.clone(),
                    created_at: commit.date.clone(),
                    is_base_merge,
                    has_conflict_changes,
                }]
            }
        })
        .collect();

    Ok(entries)
}

/// Iterate event entries oldest-to-newest. For each base-branch merge commit that
/// has no event yet (`id: None`), check if every older commit already has an event.
/// If so, auto-create a commit event, propagate xrefs from the previous event
/// (with `skip_file_translation: true`), and advance `head_event_id`.
///
/// This ensures that once a user reviews up to a merge commit, the merge gets
/// an event automatically so the xref chain remains unbroken for the next real review.
fn lazy_create_base_merge_events(
    conn: &mut SqliteConnection,
    entries: &mut [EventEntry],
    project_id: &str,
    project_path: &str,
    branch_context_id: &str,
) -> Result<(), AppError> {
    // entries are newest-first (git log order). Walk oldest-to-newest.
    // Track the most recent event id we've seen so far (for xref propagation).
    let len = entries.len();
    for i in (0..len).rev() {
        if !entries[i].is_base_merge || entries[i].id.is_some() || entries[i].has_conflict_changes {
            continue;
        }

        // Check that ALL entries older than this one (indices i+1..len, since
        // entries are newest-first) have events.
        let all_older_have_events = entries[i + 1..].iter().all(|e| e.id.is_some());
        if !all_older_have_events {
            continue;
        }

        // Find the previous event (the next entry in the array, which is one
        // commit older) to propagate xrefs from. If this merge is the oldest
        // commit on the branch, there's no previous event and no xrefs to propagate.
        let prev_event_id = if i + 1 < len {
            entries[i + 1].id.clone()
        } else {
            None
        };

        let commit_hash = entries[i].commit.clone();

        // Create the event for this merge commit
        let event = event_repo::create(
            conn,
            NewEvent::new(
                project_id.to_string(),
                "commit".to_string(),
                Some(commit_hash.clone()),
                entries[i].message.clone(),
            ),
        )?;
        let new_event_id = event.id.clone();

        // Propagate xrefs from the previous event, skipping file translation
        // since base-branch merge file changes aren't feature-branch work.
        if let Some(ref prev_id) = prev_event_id {
            let prev_event = event_repo::get(conn, prev_id)?;
            let prev_commit = prev_event.hash.as_deref().unwrap_or(&commit_hash);
            propagate_previous_xrefs(
                conn,
                PropagateXrefsParams {
                    project_id,
                    new_event_id: &new_event_id,
                    prev_event_id: prev_id,
                    prev_commit,
                    resolved_issues: &[],
                    project_path,
                    commit: &commit_hash,
                    branch_context_id,
                    skip_file_translation: true,
                },
            )?;
        }

        // Advance head_event_id to the newly created merge event
        branch_context_repo::update(
            conn,
            branch_context_id,
            branch_context::head_event_id.eq(&new_event_id),
        )?;

        // Update the entry in-place so the frontend sees the created event
        entries[i].id = Some(new_event_id);
    }

    Ok(())
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
