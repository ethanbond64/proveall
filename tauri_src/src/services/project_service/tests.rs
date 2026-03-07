use crate::commands::event_commands::{NewIssueInput, NewIssueReviewInput};
use crate::services::event_service::create_event;
use crate::services::project_service::get_project_state;
use crate::test_utils::*;
use std::process::Command;

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
    let (project_id, bc_id) =
        setup_db_records_with_branch(&mut conn, repo_path, "feature", "main");

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
    let (project_id, bc_id) =
        setup_db_records_with_branch(&mut conn, repo_path, "feature", "main");

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
    let (project_id, bc_id) =
        setup_db_records_with_branch(&mut conn, repo_path, "feature", "main");

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
    let (project_id, bc_id) =
        setup_db_records_with_branch(&mut conn, repo_path, "feature", "main");

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
    let (project_id, bc_id) =
        setup_db_records_with_branch(&mut conn, repo_path, "feature", "main");

    // Two commits, neither reviewed
    std::fs::write(dir.path().join("a.txt"), "a\n").unwrap();
    git_commit(&dir, "first feature commit");

    std::fs::write(dir.path().join("b.txt"), "b\n").unwrap();
    git_commit(&dir, "second feature commit");

    let result = get_project_state(&mut conn, &project_id, &bc_id).unwrap();

    // Should have 2 event entries (one per commit), both unreviewed (no event id)
    assert_eq!(result.events.len(), 2);
    for entry in &result.events {
        assert!(entry.id.is_none(), "unreviewed commits should have no event id");
        assert_eq!(entry.event_type, "commit");
    }
}

#[test]
fn test_events_include_reviewed_commits() {
    let mut conn = test_conn();
    let (dir, _base_hash) = setup_git_repo();
    let repo_path = dir.path().to_str().unwrap();

    setup_feature_branch(&dir);
    let (project_id, bc_id) =
        setup_db_records_with_branch(&mut conn, repo_path, "feature", "main");

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
    let (project_id, bc_id) =
        setup_db_records_with_branch(&mut conn, repo_path, "feature", "main");

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
    let (project_id, bc_id) =
        setup_db_records_with_branch(&mut conn, repo_path, "feature", "main");

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
