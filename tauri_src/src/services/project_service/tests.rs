use crate::commands::event_commands::{NewIssueInput, NewIssueReviewInput};
use crate::models::composite_file_review_state::ReviewSummaryMetadataEntry;
use crate::repositories::{branch_context_repo, event_issue_composite_xref_repo};
use crate::services::event_service::create_event;
use crate::services::project_service::get_project_state;
use crate::test_utils::*;
use std::process::Command;
use tempfile::TempDir;

// ---------------------------------------------------------------------------
// On base branch — returns zeroed response
// ---------------------------------------------------------------------------

#[test]
fn test_on_base_branch_returns_empty_state() {
    let mut conn = test_conn();
    let (dir, _base_hash) = setup_git_repo();
    let repo_path = dir.path().to_str().unwrap();
    let (project_id, bc_id) = setup_db_records(&mut conn, repo_path);

    // HEAD is on main, which is the base_branch
    let result = get_project_state(&mut conn, &project_id, &bc_id).unwrap();

    assert_eq!(result.branch_data.branch, "main");
    assert_eq!(result.branch_data.base_branch, "main");
    assert_eq!(result.diff_summary.files_added, 0);
    assert_eq!(result.diff_summary.files_modified, 0);
    assert_eq!(result.diff_summary.files_deleted, 0);
    assert_eq!(result.diff_summary.lines_added, 0);
    assert_eq!(result.diff_summary.lines_deleted, 0);
    assert!(result.events.is_empty());
    assert!(result.issues.is_empty());
}

// ---------------------------------------------------------------------------
// Feature branch with added, modified, and deleted files
// ---------------------------------------------------------------------------

#[test]
fn test_feature_branch_diff_summary_added_file() {
    let mut conn = test_conn();
    let (dir, _base_hash) = setup_git_repo();
    let repo_path = dir.path().to_str().unwrap();

    setup_feature_branch(&dir);
    let (project_id, bc_id) = setup_db_records_with_branch(&mut conn, repo_path, "feature", "main");

    // Add a new file on the feature branch
    std::fs::write(dir.path().join("new.txt"), "hello\nworld\n").unwrap();
    git_commit(&dir, "add new.txt");

    let result = get_project_state(&mut conn, &project_id, &bc_id).unwrap();

    assert_eq!(result.branch_data.branch, "feature");
    assert_eq!(result.branch_data.base_branch, "main");
    assert_eq!(result.diff_summary.files_added, 1);
    assert_eq!(result.diff_summary.files_modified, 0);
    assert_eq!(result.diff_summary.files_deleted, 0);
    assert_eq!(result.diff_summary.lines_added, 2);
    assert_eq!(result.diff_summary.lines_deleted, 0);
}

#[test]
fn test_feature_branch_diff_summary_modified_file() {
    let mut conn = test_conn();
    let (dir, _base_hash) = setup_git_repo();
    let repo_path = dir.path().to_str().unwrap();

    setup_feature_branch(&dir);
    let (project_id, bc_id) = setup_db_records_with_branch(&mut conn, repo_path, "feature", "main");

    // Modify base.txt (replace "base\n" with "modified\nextra\n")
    std::fs::write(dir.path().join("base.txt"), "modified\nextra\n").unwrap();
    git_commit(&dir, "modify base.txt");

    let result = get_project_state(&mut conn, &project_id, &bc_id).unwrap();

    assert_eq!(result.diff_summary.files_added, 0);
    assert_eq!(result.diff_summary.files_modified, 1);
    assert_eq!(result.diff_summary.files_deleted, 0);
    // "base\n" (1 line) replaced by "modified\nextra\n" (2 lines)
    assert_eq!(result.diff_summary.lines_added, 2);
    assert_eq!(result.diff_summary.lines_deleted, 1);
}

#[test]
fn test_feature_branch_diff_summary_deleted_file() {
    let mut conn = test_conn();
    let (dir, _base_hash) = setup_git_repo();
    let repo_path = dir.path().to_str().unwrap();

    setup_feature_branch(&dir);
    let (project_id, bc_id) = setup_db_records_with_branch(&mut conn, repo_path, "feature", "main");

    // Delete base.txt
    Command::new("git")
        .args(["rm", "base.txt"])
        .current_dir(dir.path())
        .output()
        .unwrap();
    git_commit(&dir, "delete base.txt");

    let result = get_project_state(&mut conn, &project_id, &bc_id).unwrap();

    assert_eq!(result.diff_summary.files_added, 0);
    assert_eq!(result.diff_summary.files_modified, 0);
    assert_eq!(result.diff_summary.files_deleted, 1);
    assert_eq!(result.diff_summary.lines_added, 0);
    assert_eq!(result.diff_summary.lines_deleted, 1);
}

#[test]
fn test_feature_branch_diff_summary_mixed() {
    let mut conn = test_conn();
    let (dir, _base_hash) = setup_git_repo();
    let repo_path = dir.path().to_str().unwrap();

    setup_feature_branch(&dir);
    let (project_id, bc_id) = setup_db_records_with_branch(&mut conn, repo_path, "feature", "main");

    // Add a file, modify base.txt, in one commit
    std::fs::write(dir.path().join("added.txt"), "new\n").unwrap();
    std::fs::write(dir.path().join("base.txt"), "changed\n").unwrap();
    git_commit(&dir, "add and modify");

    let result = get_project_state(&mut conn, &project_id, &bc_id).unwrap();

    assert_eq!(result.diff_summary.files_added, 1);
    assert_eq!(result.diff_summary.files_modified, 1);
    assert_eq!(result.diff_summary.files_deleted, 0);
}

// ---------------------------------------------------------------------------
// Event entries
// ---------------------------------------------------------------------------

#[test]
fn test_events_include_unreviewed_commits() {
    let mut conn = test_conn();
    let (dir, _base_hash) = setup_git_repo();
    let repo_path = dir.path().to_str().unwrap();

    setup_feature_branch(&dir);
    let (project_id, bc_id) = setup_db_records_with_branch(&mut conn, repo_path, "feature", "main");

    // Two commits, neither reviewed
    std::fs::write(dir.path().join("a.txt"), "a\n").unwrap();
    git_commit(&dir, "first feature commit");

    std::fs::write(dir.path().join("b.txt"), "b\n").unwrap();
    git_commit(&dir, "second feature commit");

    let result = get_project_state(&mut conn, &project_id, &bc_id).unwrap();

    // Should have 2 event entries (one per commit), both unreviewed (no event id)
    assert_eq!(result.events.len(), 2);
    for entry in &result.events {
        assert!(
            entry.id.is_none(),
            "unreviewed commits should have no event id"
        );
        assert_eq!(entry.event_type, "commit");
    }
}

#[test]
fn test_events_include_reviewed_commits() {
    let mut conn = test_conn();
    let (dir, _base_hash) = setup_git_repo();
    let repo_path = dir.path().to_str().unwrap();

    setup_feature_branch(&dir);
    let (project_id, bc_id) = setup_db_records_with_branch(&mut conn, repo_path, "feature", "main");

    std::fs::write(dir.path().join("a.txt"), "a\n").unwrap();
    let c1 = git_commit(&dir, "feature commit");

    // Review c1
    create_event(
        &mut conn,
        &project_id,
        c1,
        "commit".to_string(),
        vec![],
        vec![],
        &bc_id,
    )
    .unwrap();

    let result = get_project_state(&mut conn, &project_id, &bc_id).unwrap();

    assert_eq!(result.events.len(), 1);
    assert!(
        result.events[0].id.is_some(),
        "reviewed commit should have an event id"
    );
}

// ---------------------------------------------------------------------------
// Issues
// ---------------------------------------------------------------------------

#[test]
fn test_issues_listed_when_present() {
    let mut conn = test_conn();
    let (dir, _base_hash) = setup_git_repo();
    let repo_path = dir.path().to_str().unwrap();

    setup_feature_branch(&dir);
    let (project_id, bc_id) = setup_db_records_with_branch(&mut conn, repo_path, "feature", "main");

    std::fs::write(dir.path().join("code.txt"), "a\nb\nc\n").unwrap();
    let c1 = git_commit(&dir, "add code.txt");

    // Review with an issue
    create_event(
        &mut conn,
        &project_id,
        c1,
        "commit".to_string(),
        vec![NewIssueInput {
            comment: "needs work".to_string(),
            reviews: vec![NewIssueReviewInput {
                type_: "line".to_string(),
                path: Some("code.txt".to_string()),
                start: Some(1),
                end: Some(2),
                state: "red".to_string(),
            }],
        }],
        vec![],
        &bc_id,
    )
    .unwrap();

    let result = get_project_state(&mut conn, &project_id, &bc_id).unwrap();

    assert_eq!(result.issues.len(), 1);
    assert_eq!(result.issues[0].comment, "needs work");
}

#[test]
fn test_no_issues_when_none_filed() {
    let mut conn = test_conn();
    let (dir, _base_hash) = setup_git_repo();
    let repo_path = dir.path().to_str().unwrap();

    setup_feature_branch(&dir);
    let (project_id, bc_id) = setup_db_records_with_branch(&mut conn, repo_path, "feature", "main");

    std::fs::write(dir.path().join("a.txt"), "a\n").unwrap();
    let c1 = git_commit(&dir, "add a.txt");

    // Review without issues
    create_event(
        &mut conn,
        &project_id,
        c1,
        "commit".to_string(),
        vec![],
        vec![],
        &bc_id,
    )
    .unwrap();

    let result = get_project_state(&mut conn, &project_id, &bc_id).unwrap();

    assert!(result.issues.is_empty());
}

// ---------------------------------------------------------------------------
// Lazy-creation of merge commit events
// ---------------------------------------------------------------------------

/// Helper: set up a feature branch with one commit, then merge main into it.
/// Returns (c1_hash, merge_hash).
/// The merge is a no-conflict base-branch merge (main changed a different file).
fn setup_feature_with_base_merge(dir: &TempDir) -> (String, String) {
    // On feature: add a file and commit
    std::fs::write(dir.path().join("feature.txt"), "line1\nline2\nline3\n").unwrap();
    let c1 = git_commit(dir, "add feature.txt");

    // Switch to main, make a change, switch back and merge
    git_checkout(dir, "main");
    std::fs::write(dir.path().join("main_update.txt"), "from main\n").unwrap();
    git_commit(dir, "main: add main_update.txt");

    git_checkout(dir, "feature");
    let merge_hash = git_merge(dir, "main");

    (c1, merge_hash)
}

#[test]
fn test_base_merge_shown_in_events() {
    let mut conn = test_conn();
    let (dir, _) = setup_git_repo();
    let repo_path = dir.path().to_str().unwrap();
    setup_feature_branch(&dir);
    let (project_id, bc_id) = setup_db_records_with_branch(&mut conn, repo_path, "feature", "main");

    let (_c1, merge_hash) = setup_feature_with_base_merge(&dir);

    let result = get_project_state(&mut conn, &project_id, &bc_id).unwrap();

    // Should have 2 entries: merge (newest) and c1 (oldest)
    assert_eq!(result.events.len(), 2);

    let merge_entry = result
        .events
        .iter()
        .find(|e| e.commit == merge_hash)
        .unwrap();
    assert!(
        merge_entry.is_base_merge,
        "merge commit should be classified as base merge"
    );
    assert!(
        !merge_entry.has_conflict_changes,
        "clean merge should have no conflict changes"
    );
}

#[test]
fn test_base_merge_not_lazy_created_when_prior_commits_unreviewed() {
    let mut conn = test_conn();
    let (dir, _) = setup_git_repo();
    let repo_path = dir.path().to_str().unwrap();
    setup_feature_branch(&dir);
    let (project_id, bc_id) = setup_db_records_with_branch(&mut conn, repo_path, "feature", "main");

    let (_c1, merge_hash) = setup_feature_with_base_merge(&dir);

    // Don't review c1 — merge event should NOT be lazy-created
    let result = get_project_state(&mut conn, &project_id, &bc_id).unwrap();

    let merge_entry = result
        .events
        .iter()
        .find(|e| e.commit == merge_hash)
        .unwrap();
    assert!(
        merge_entry.id.is_none(),
        "merge should not have event when prior commit is unreviewed"
    );
}

#[test]
fn test_base_merge_lazy_created_when_prior_commits_reviewed() {
    let mut conn = test_conn();
    let (dir, _) = setup_git_repo();
    let repo_path = dir.path().to_str().unwrap();
    setup_feature_branch(&dir);
    let (project_id, bc_id) = setup_db_records_with_branch(&mut conn, repo_path, "feature", "main");

    let (c1, merge_hash) = setup_feature_with_base_merge(&dir);

    // Review c1
    create_event(
        &mut conn,
        &project_id,
        c1,
        "commit".to_string(),
        vec![],
        vec![],
        &bc_id,
    )
    .unwrap();

    // Now get_project_state should lazy-create the merge event
    let result = get_project_state(&mut conn, &project_id, &bc_id).unwrap();

    let merge_entry = result
        .events
        .iter()
        .find(|e| e.commit == merge_hash)
        .unwrap();
    assert!(
        merge_entry.id.is_some(),
        "merge event should be lazy-created after prior commit is reviewed"
    );
}

#[test]
fn test_base_merge_lazy_creation_advances_head_event_id() {
    let mut conn = test_conn();
    let (dir, _) = setup_git_repo();
    let repo_path = dir.path().to_str().unwrap();
    setup_feature_branch(&dir);
    let (project_id, bc_id) = setup_db_records_with_branch(&mut conn, repo_path, "feature", "main");

    let (c1, merge_hash) = setup_feature_with_base_merge(&dir);

    // Review c1
    create_event(
        &mut conn,
        &project_id,
        c1,
        "commit".to_string(),
        vec![],
        vec![],
        &bc_id,
    )
    .unwrap();

    // Trigger lazy creation
    let result = get_project_state(&mut conn, &project_id, &bc_id).unwrap();

    let merge_event_id = result
        .events
        .iter()
        .find(|e| e.commit == merge_hash)
        .unwrap()
        .id
        .as_ref()
        .expect("merge event should exist");

    // head_event_id should now point to the merge event
    let bc = branch_context_repo::get(&mut conn, &bc_id).unwrap();
    assert_eq!(bc.head_event_id.as_deref(), Some(merge_event_id.as_str()));
}

#[test]
fn test_base_merge_lazy_creation_propagates_xrefs() {
    // Setup: feature commit with an issue on feature.txt lines 1-2,
    // then a clean merge from main. After lazy-creation, the merge event's
    // xrefs should carry forward the same issue, file, and line range.
    let mut conn = test_conn();
    let (dir, _) = setup_git_repo();
    let repo_path = dir.path().to_str().unwrap();
    setup_feature_branch(&dir);
    let (project_id, bc_id) = setup_db_records_with_branch(&mut conn, repo_path, "feature", "main");

    // Feature commit: add feature.txt
    std::fs::write(dir.path().join("feature.txt"), "line1\nline2\nline3\n").unwrap();
    let c1 = git_commit(&dir, "add feature.txt");

    // Review c1 with an issue on feature.txt lines 1-2
    create_event(
        &mut conn,
        &project_id,
        c1,
        "commit".to_string(),
        vec![NewIssueInput {
            comment: "needs refactor".to_string(),
            reviews: vec![NewIssueReviewInput {
                type_: "line".to_string(),
                path: Some("feature.txt".to_string()),
                start: Some(1),
                end: Some(2),
                state: "red".to_string(),
            }],
        }],
        vec![],
        &bc_id,
    )
    .unwrap();

    // Verify xrefs exist on the c1 event
    let bc_before = branch_context_repo::get(&mut conn, &bc_id).unwrap();
    let c1_event_id = bc_before.head_event_id.as_ref().unwrap();
    let c1_xrefs =
        event_issue_composite_xref_repo::join_list_by_event(&mut conn, c1_event_id, &bc_id)
            .unwrap();
    assert_eq!(c1_xrefs.len(), 1, "c1 should have exactly one xref");
    let (c1_xref, c1_composite) = &c1_xrefs[0];
    let c1_issue_id = c1_xref.issue_id.clone();
    assert_eq!(c1_composite.relative_file_path, "feature.txt");
    let c1_entries: Vec<ReviewSummaryMetadataEntry> =
        serde_json::from_str(&c1_composite.summary_metadata).unwrap();
    assert_eq!(c1_entries.len(), 1);
    assert_eq!(c1_entries[0].start, 1);
    assert_eq!(c1_entries[0].end, 2);
    assert_eq!(c1_entries[0].state, "red");

    // Now merge main into feature (main changed a different file, no conflict)
    git_checkout(&dir, "main");
    std::fs::write(dir.path().join("main_only.txt"), "from main\n").unwrap();
    git_commit(&dir, "main: add main_only.txt");
    git_checkout(&dir, "feature");
    let merge_hash = git_merge(&dir, "main");

    // Trigger lazy creation via get_project_state
    let result = get_project_state(&mut conn, &project_id, &bc_id).unwrap();

    // Merge event should be lazy-created
    let merge_entry = result
        .events
        .iter()
        .find(|e| e.commit == merge_hash)
        .unwrap();
    let merge_event_id = merge_entry
        .id
        .as_ref()
        .expect("merge event should be lazy-created");

    // Verify xrefs on the merge event
    let merge_xrefs =
        event_issue_composite_xref_repo::join_list_by_event(&mut conn, merge_event_id, &bc_id)
            .unwrap();
    assert_eq!(
        merge_xrefs.len(),
        1,
        "merge event should have exactly one xref (propagated from c1)"
    );

    let (merge_xref, merge_composite) = &merge_xrefs[0];

    // Same issue, same file
    assert_eq!(
        merge_xref.issue_id, c1_issue_id,
        "xref should reference the same issue"
    );
    assert_eq!(
        merge_composite.relative_file_path, "feature.txt",
        "xref should reference the same file"
    );

    // Same line range (skip_file_translation=true means no translation)
    let merge_entries: Vec<ReviewSummaryMetadataEntry> =
        serde_json::from_str(&merge_composite.summary_metadata).unwrap();
    assert_eq!(merge_entries.len(), 1);
    assert_eq!(
        merge_entries[0].start, 1,
        "line start should be unchanged across merge"
    );
    assert_eq!(
        merge_entries[0].end, 2,
        "line end should be unchanged across merge"
    );
    assert_eq!(
        merge_entries[0].state, "red",
        "review state should be unchanged across merge"
    );
}

#[test]
fn test_base_merge_xrefs_reuse_composite_not_translate() {
    // When skip_file_translation is true, the merge event should reuse the
    // exact same composite_file_id (not create a new one with translated lines).
    let mut conn = test_conn();
    let (dir, _) = setup_git_repo();
    let repo_path = dir.path().to_str().unwrap();
    setup_feature_branch(&dir);
    let (project_id, bc_id) = setup_db_records_with_branch(&mut conn, repo_path, "feature", "main");

    std::fs::write(dir.path().join("feature.txt"), "line1\nline2\nline3\n").unwrap();
    let c1 = git_commit(&dir, "add feature.txt");

    create_event(
        &mut conn,
        &project_id,
        c1,
        "commit".to_string(),
        vec![NewIssueInput {
            comment: "issue".to_string(),
            reviews: vec![NewIssueReviewInput {
                type_: "line".to_string(),
                path: Some("feature.txt".to_string()),
                start: Some(1),
                end: Some(3),
                state: "yellow".to_string(),
            }],
        }],
        vec![],
        &bc_id,
    )
    .unwrap();

    let bc = branch_context_repo::get(&mut conn, &bc_id).unwrap();
    let c1_event_id = bc.head_event_id.unwrap();
    let c1_xrefs =
        event_issue_composite_xref_repo::join_list_by_event(&mut conn, &c1_event_id, &bc_id)
            .unwrap();
    let c1_composite_id = c1_xrefs[0].0.composite_file_id.clone();

    // Merge main (different file) into feature
    git_checkout(&dir, "main");
    std::fs::write(dir.path().join("other.txt"), "other\n").unwrap();
    git_commit(&dir, "main: add other.txt");
    git_checkout(&dir, "feature");
    git_merge(&dir, "main");

    // Trigger lazy creation
    get_project_state(&mut conn, &project_id, &bc_id).unwrap();

    let bc = branch_context_repo::get(&mut conn, &bc_id).unwrap();
    let merge_event_id = bc.head_event_id.unwrap();
    let merge_xrefs =
        event_issue_composite_xref_repo::join_list_by_event(&mut conn, &merge_event_id, &bc_id)
            .unwrap();

    // The composite should be reused (same ID), not a new translated copy
    assert_eq!(
        merge_xrefs[0].0.composite_file_id, c1_composite_id,
        "merge xref should reuse the same composite (skip_file_translation=true)"
    );
}

#[test]
fn test_base_merge_lazy_creation_issues_visible_in_response() {
    // After lazy-creating a merge event, the issues from the previous event
    // should be visible in the get_project_state response (since head_event_id
    // now points to the merge event with propagated xrefs).
    let mut conn = test_conn();
    let (dir, _) = setup_git_repo();
    let repo_path = dir.path().to_str().unwrap();
    setup_feature_branch(&dir);
    let (project_id, bc_id) = setup_db_records_with_branch(&mut conn, repo_path, "feature", "main");

    std::fs::write(dir.path().join("code.txt"), "a\nb\nc\n").unwrap();
    let c1 = git_commit(&dir, "add code.txt");

    create_event(
        &mut conn,
        &project_id,
        c1,
        "commit".to_string(),
        vec![NewIssueInput {
            comment: "bug in code.txt".to_string(),
            reviews: vec![NewIssueReviewInput {
                type_: "line".to_string(),
                path: Some("code.txt".to_string()),
                start: Some(2),
                end: Some(3),
                state: "red".to_string(),
            }],
        }],
        vec![],
        &bc_id,
    )
    .unwrap();

    // Merge main into feature
    git_checkout(&dir, "main");
    std::fs::write(dir.path().join("readme.txt"), "readme\n").unwrap();
    git_commit(&dir, "main: add readme");
    git_checkout(&dir, "feature");
    git_merge(&dir, "main");

    let result = get_project_state(&mut conn, &project_id, &bc_id).unwrap();

    // The issue should still be visible after the merge event becomes head
    assert_eq!(result.issues.len(), 1);
    assert_eq!(result.issues[0].comment, "bug in code.txt");
}

#[test]
fn test_conflicted_base_merge_not_lazy_created() {
    // Bug A: A conflicted merge should NOT be auto-reviewed by lazy_create_base_merge_events
    // even when all prior commits have events.
    let mut conn = test_conn();
    let (dir, _) = setup_git_repo();
    let repo_path = dir.path().to_str().unwrap();
    setup_feature_branch(&dir);
    let (project_id, bc_id) = setup_db_records_with_branch(&mut conn, repo_path, "feature", "main");

    // Feature commit
    std::fs::write(dir.path().join("base.txt"), "feature version\n").unwrap();
    let c1 = git_commit(&dir, "modify base.txt on feature");

    // Main also modifies base.txt → conflict
    git_checkout(&dir, "main");
    std::fs::write(dir.path().join("base.txt"), "main version\n").unwrap();
    git_commit(&dir, "main: modify base.txt");
    git_checkout(&dir, "feature");
    let merge_hash = git_merge_with_conflict(&dir, "main");

    // Review c1 — all prior commits now have events
    create_event(
        &mut conn,
        &project_id,
        c1,
        "commit".to_string(),
        vec![],
        vec![],
        &bc_id,
    )
    .unwrap();

    // get_project_state should NOT lazy-create the conflicted merge event
    let result = get_project_state(&mut conn, &project_id, &bc_id).unwrap();

    let merge_entry = result
        .events
        .iter()
        .find(|e| e.commit == merge_hash)
        .unwrap();
    assert!(
        merge_entry.id.is_none(),
        "conflicted merge should NOT be lazy-created"
    );
    assert!(
        merge_entry.has_conflict_changes,
        "should have conflict changes"
    );
}

#[test]
fn test_multiple_base_merges_lazy_created_in_sequence() {
    // c1 (reviewed) -> merge1 -> c2 (reviewed) -> merge2
    // Both merges should be lazy-created in order.
    let mut conn = test_conn();
    let (dir, _) = setup_git_repo();
    let repo_path = dir.path().to_str().unwrap();
    setup_feature_branch(&dir);
    let (project_id, bc_id) = setup_db_records_with_branch(&mut conn, repo_path, "feature", "main");

    // c1 on feature
    std::fs::write(dir.path().join("f1.txt"), "f1\n").unwrap();
    let c1 = git_commit(&dir, "feature: add f1.txt");

    // Review c1
    create_event(
        &mut conn,
        &project_id,
        c1,
        "commit".to_string(),
        vec![],
        vec![],
        &bc_id,
    )
    .unwrap();

    // merge1: merge main into feature
    git_checkout(&dir, "main");
    std::fs::write(dir.path().join("m1.txt"), "m1\n").unwrap();
    git_commit(&dir, "main: add m1.txt");
    git_checkout(&dir, "feature");
    let merge1 = git_merge(&dir, "main");

    // c2 on feature
    std::fs::write(dir.path().join("f2.txt"), "f2\n").unwrap();
    let c2 = git_commit(&dir, "feature: add f2.txt");

    // Review c2 (this triggers intermediate event creation for merge1 via event_service)
    create_event(
        &mut conn,
        &project_id,
        c2,
        "commit".to_string(),
        vec![],
        vec![],
        &bc_id,
    )
    .unwrap();

    // merge2: merge main into feature again
    git_checkout(&dir, "main");
    std::fs::write(dir.path().join("m2.txt"), "m2\n").unwrap();
    git_commit(&dir, "main: add m2.txt");
    git_checkout(&dir, "feature");
    let merge2 = git_merge(&dir, "main");

    // get_project_state should lazy-create merge2
    let result = get_project_state(&mut conn, &project_id, &bc_id).unwrap();

    // merge1 should have been created by create_event's intermediate logic
    let merge1_entry = result.events.iter().find(|e| e.commit == merge1).unwrap();
    assert!(
        merge1_entry.id.is_some(),
        "merge1 should have event from intermediate creation"
    );

    // merge2 should be lazy-created by get_project_state
    let merge2_entry = result.events.iter().find(|e| e.commit == merge2).unwrap();
    assert!(merge2_entry.id.is_some(), "merge2 should be lazy-created");
}
