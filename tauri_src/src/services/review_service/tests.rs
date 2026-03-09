use crate::commands::event_commands::{NewIssueInput, NewIssueReviewInput};
use crate::repositories::issue_repo;
use crate::services::event_service::create_event;
use crate::services::review_service::{get_review_file_data, get_review_file_system_data};
use crate::test_utils::*;
use diesel::sqlite::SqliteConnection;
use std::process::Command;
use tempfile::TempDir;

// ---------------------------------------------------------------------------
// get_review_file_system_data tests
// ---------------------------------------------------------------------------

#[test]
fn test_file_system_data_added_file() {
    let mut conn = test_conn();
    let (dir, base_hash) = setup_git_repo();
    let repo_path = dir.path().to_str().unwrap();
    let (project_id, bc_id) = setup_db_records(&mut conn, repo_path);

    // Review the base commit to set head_event_id
    create_event(
        &mut conn,
        &project_id,
        base_hash,
        "commit".to_string(),
        vec![],
        vec![],
        &bc_id,
    )
    .unwrap();

    // Add a new file and commit
    std::fs::write(dir.path().join("new_file.txt"), "hello\n").unwrap();
    let c1 = git_commit(&dir, "add new_file.txt");

    // Query file system data for c1 BEFORE reviewing it
    let result = get_review_file_system_data(
        &mut conn,
        &project_id,
        c1,
        None,
        "commit".to_string(),
        &bc_id,
    )
    .unwrap();

    let added = result
        .touched_files
        .iter()
        .find(|f| f.path == "new_file.txt")
        .expect("new_file.txt should be in touched files");
    assert_eq!(added.diff_mode.as_deref(), Some("A"));
    assert_eq!(added.name, "new_file.txt");
}

#[test]
fn test_file_system_data_deleted_file() {
    let mut conn = test_conn();
    let (dir, base_hash) = setup_git_repo();
    let repo_path = dir.path().to_str().unwrap();
    let (project_id, bc_id) = setup_db_records(&mut conn, repo_path);

    // Review the base commit
    create_event(
        &mut conn,
        &project_id,
        base_hash,
        "commit".to_string(),
        vec![],
        vec![],
        &bc_id,
    )
    .unwrap();

    // Delete base.txt and commit
    Command::new("git")
        .args(["rm", "base.txt"])
        .current_dir(dir.path())
        .output()
        .unwrap();
    let c1 = git_commit(&dir, "delete base.txt");

    let result = get_review_file_system_data(
        &mut conn,
        &project_id,
        c1,
        None,
        "commit".to_string(),
        &bc_id,
    )
    .unwrap();

    let deleted = result
        .touched_files
        .iter()
        .find(|f| f.path == "base.txt")
        .expect("base.txt should be in touched files");
    assert_eq!(deleted.diff_mode.as_deref(), Some("D"));
}

#[test]
fn test_file_system_data_modified_file() {
    let mut conn = test_conn();
    let (dir, base_hash) = setup_git_repo();
    let repo_path = dir.path().to_str().unwrap();
    let (project_id, bc_id) = setup_db_records(&mut conn, repo_path);

    // Review the base commit
    create_event(
        &mut conn,
        &project_id,
        base_hash,
        "commit".to_string(),
        vec![],
        vec![],
        &bc_id,
    )
    .unwrap();

    // Modify base.txt and commit
    std::fs::write(dir.path().join("base.txt"), "modified content\n").unwrap();
    let c1 = git_commit(&dir, "modify base.txt");

    let result = get_review_file_system_data(
        &mut conn,
        &project_id,
        c1,
        None,
        "commit".to_string(),
        &bc_id,
    )
    .unwrap();

    let modified = result
        .touched_files
        .iter()
        .find(|f| f.path == "base.txt")
        .expect("base.txt should be in touched files");
    assert_eq!(modified.diff_mode.as_deref(), Some("M"));
}

#[test]
fn test_file_system_data_mixed_changes() {
    let mut conn = test_conn();
    let (dir, base_hash) = setup_git_repo();
    let repo_path = dir.path().to_str().unwrap();
    let (project_id, bc_id) = setup_db_records(&mut conn, repo_path);

    // Review the base commit
    create_event(
        &mut conn,
        &project_id,
        base_hash,
        "commit".to_string(),
        vec![],
        vec![],
        &bc_id,
    )
    .unwrap();

    // In one commit: modify base.txt and add a new file
    std::fs::write(dir.path().join("base.txt"), "changed\n").unwrap();
    std::fs::write(dir.path().join("added.txt"), "new\n").unwrap();
    let c1 = git_commit(&dir, "modify and add");

    let result = get_review_file_system_data(
        &mut conn,
        &project_id,
        c1,
        None,
        "commit".to_string(),
        &bc_id,
    )
    .unwrap();

    assert_eq!(result.touched_files.len(), 2);

    let modes: Vec<_> = result
        .touched_files
        .iter()
        .map(|f| (f.path.as_str(), f.diff_mode.as_deref()))
        .collect();
    assert!(modes.contains(&("added.txt", Some("A"))));
    assert!(modes.contains(&("base.txt", Some("M"))));
}

// ---------------------------------------------------------------------------
// get_review_file_data tests
// ---------------------------------------------------------------------------

#[test]
fn test_file_data_added_file_content() {
    let mut conn = test_conn();
    let (dir, base_hash) = setup_git_repo();
    let repo_path = dir.path().to_str().unwrap();
    let (project_id, bc_id) = setup_db_records(&mut conn, repo_path);

    create_event(
        &mut conn,
        &project_id,
        base_hash,
        "commit".to_string(),
        vec![],
        vec![],
        &bc_id,
    )
    .unwrap();

    // Add a new file
    std::fs::write(dir.path().join("new.txt"), "line1\nline2\n").unwrap();
    let c1 = git_commit(&dir, "add new.txt");

    let result = get_review_file_data(
        &mut conn,
        &project_id,
        c1,
        None,
        "commit".to_string(),
        "new.txt".to_string(),
        &bc_id,
    )
    .unwrap();

    // Content is the file at this commit
    assert_eq!(result.content, "line1\nline2\n");
    // Diff (base version) is empty — file didn't exist at the base ref
    assert_eq!(result.diff.as_deref(), Some(""));
}

#[test]
fn test_file_data_modified_file_shows_old_content_as_diff() {
    let mut conn = test_conn();
    let (dir, base_hash) = setup_git_repo();
    let repo_path = dir.path().to_str().unwrap();
    let (project_id, bc_id) = setup_db_records(&mut conn, repo_path);

    create_event(
        &mut conn,
        &project_id,
        base_hash,
        "commit".to_string(),
        vec![],
        vec![],
        &bc_id,
    )
    .unwrap();

    // Modify base.txt
    std::fs::write(dir.path().join("base.txt"), "updated\n").unwrap();
    let c1 = git_commit(&dir, "modify base.txt");

    let result = get_review_file_data(
        &mut conn,
        &project_id,
        c1,
        None,
        "commit".to_string(),
        "base.txt".to_string(),
        &bc_id,
    )
    .unwrap();

    // Content is the new version
    assert_eq!(result.content, "updated\n");
    // Diff is the old version (base content)
    assert_eq!(result.diff.as_deref(), Some("base\n"));
}

#[test]
fn test_file_data_deleted_file() {
    let mut conn = test_conn();
    let (dir, base_hash) = setup_git_repo();
    let repo_path = dir.path().to_str().unwrap();
    let (project_id, bc_id) = setup_db_records(&mut conn, repo_path);

    create_event(
        &mut conn,
        &project_id,
        base_hash,
        "commit".to_string(),
        vec![],
        vec![],
        &bc_id,
    )
    .unwrap();

    // Delete base.txt
    Command::new("git")
        .args(["rm", "base.txt"])
        .current_dir(dir.path())
        .output()
        .unwrap();
    let c1 = git_commit(&dir, "delete base.txt");

    let result = get_review_file_data(
        &mut conn,
        &project_id,
        c1,
        None,
        "commit".to_string(),
        "base.txt".to_string(),
        &bc_id,
    )
    .unwrap();

    // Content is empty (file doesn't exist at this commit)
    assert_eq!(result.content, "");
    // Diff (old version) has the original content
    assert_eq!(result.diff.as_deref(), Some("base\n"));
}

#[test]
fn test_file_data_with_issue_returns_line_summary() {
    let mut conn = test_conn();
    let (dir, base_hash) = setup_git_repo();
    let repo_path = dir.path().to_str().unwrap();
    let (project_id, bc_id) = setup_db_records(&mut conn, repo_path);

    create_event(
        &mut conn,
        &project_id,
        base_hash,
        "commit".to_string(),
        vec![],
        vec![],
        &bc_id,
    )
    .unwrap();

    // Add a file with multiple lines
    std::fs::write(dir.path().join("code.txt"), "a\nb\nc\nd\ne\n").unwrap();
    let c1 = git_commit(&dir, "add code.txt");

    // Review c1 with an issue on code.txt
    create_event(
        &mut conn,
        &project_id,
        c1.clone(),
        "commit".to_string(),
        vec![NewIssueInput {
            comment: "needs review".to_string(),
            reviews: vec![NewIssueReviewInput {
                type_: "line".to_string(),
                path: Some("code.txt".to_string()),
                start: Some(2),
                end: Some(4),
                state: "red".to_string(),
            }],
        }],
        vec![],
        &bc_id,
    )
    .unwrap();

    // Query file data for c1 — head_event_id now points to c1's event,
    // so line_summary comes from that event's xrefs
    let result = get_review_file_data(
        &mut conn,
        &project_id,
        c1,
        None,
        "commit".to_string(),
        "code.txt".to_string(),
        &bc_id,
    )
    .unwrap();

    assert_eq!(result.content, "a\nb\nc\nd\ne\n");
    assert_eq!(result.line_summary.len(), 1);
    assert_eq!(result.line_summary[0].start, 2);
    assert_eq!(result.line_summary[0].end, 4);
    assert_eq!(result.line_summary[0].state, "red");
    assert_eq!(result.issues.len(), 1);
    assert_eq!(result.issues[0].comment, "needs review");
}

// ---------------------------------------------------------------------------
// Merge-only files in commit (bulk) mode
// ---------------------------------------------------------------------------

#[test]
fn test_commit_bulk_merge_only_files_separated() {
    // Bulk review spanning: c1 (normal) + merge (base merge with merge-only file)
    // merge-only files should have merge_only: true
    let mut conn = test_conn();
    let (dir, base_hash) = setup_git_repo();
    let repo_path = dir.path().to_str().unwrap();

    setup_feature_branch(&dir);
    let (project_id, bc_id) =
        setup_db_records_with_branch(&mut conn, repo_path, "feature", "main");

    // c1 on feature: add feature.txt
    std::fs::write(dir.path().join("feature.txt"), "feature\n").unwrap();
    let c1 = git_commit(&dir, "add feature.txt");

    // Make a change on main (different file) and merge into feature
    git_checkout(&dir, "main");
    std::fs::write(dir.path().join("main_only.txt"), "from main\n").unwrap();
    git_commit(&dir, "main: add main_only.txt");
    git_checkout(&dir, "feature");
    let merge_hash = git_merge(&dir, "main");

    // Bulk review from base..merge_hash covers c1 + merge
    let result = get_review_file_system_data(
        &mut conn,
        &project_id,
        merge_hash,
        None,
        "commit".to_string(),
        &bc_id,
    )
    .unwrap();

    // feature.txt touched by c1 (normal commit) → merge_only: false
    let feature_file = result.touched_files.iter().find(|f| f.path == "feature.txt")
        .expect("feature.txt should be in touched files");
    assert!(!feature_file.merge_only, "feature.txt is from a normal commit");

    // main_only.txt only touched by the merge → merge_only: true
    let main_file = result.touched_files.iter().find(|f| f.path == "main_only.txt")
        .expect("main_only.txt should be in touched files");
    assert!(main_file.merge_only, "main_only.txt is only from a merge commit");
}

#[test]
fn test_commit_bulk_file_in_both_merge_and_normal_not_merge_only() {
    // A file touched by both a normal commit and a merge should NOT be merge_only
    let mut conn = test_conn();
    let (dir, base_hash) = setup_git_repo();
    let repo_path = dir.path().to_str().unwrap();

    setup_feature_branch(&dir);
    let (project_id, bc_id) =
        setup_db_records_with_branch(&mut conn, repo_path, "feature", "main");

    // c1 on feature: modify base.txt
    std::fs::write(dir.path().join("base.txt"), "feature change\n").unwrap();
    let c1 = git_commit(&dir, "modify base.txt on feature");

    // Main also modifies base.txt (will appear in merge diff too)
    git_checkout(&dir, "main");
    std::fs::write(dir.path().join("base.txt"), "main change\n").unwrap();
    git_commit(&dir, "main: modify base.txt");
    git_checkout(&dir, "feature");
    let merge_hash = git_merge_with_conflict(&dir, "main");

    // Bulk review from base..merge
    let result = get_review_file_system_data(
        &mut conn,
        &project_id,
        merge_hash,
        None,
        "commit".to_string(),
        &bc_id,
    )
    .unwrap();

    // base.txt is touched by c1 (normal) AND the merge — should NOT be merge_only
    let base_file = result.touched_files.iter().find(|f| f.path == "base.txt")
        .expect("base.txt should be in touched files");
    assert!(!base_file.merge_only, "base.txt is touched by a normal commit too");
}

#[test]
fn test_commit_bulk_conflicted_merge_conflict_files_not_merge_only() {
    // Conflict resolution files from a conflicted merge should NOT be merge_only
    let mut conn = test_conn();
    let (dir, base_hash) = setup_git_repo();
    let repo_path = dir.path().to_str().unwrap();

    setup_feature_branch(&dir);
    let (project_id, bc_id) =
        setup_db_records_with_branch(&mut conn, repo_path, "feature", "main");

    // c1 on feature: add a separate file
    std::fs::write(dir.path().join("feature.txt"), "feature\n").unwrap();
    let c1 = git_commit(&dir, "add feature.txt");

    // Modify base.txt on feature to set up conflict
    std::fs::write(dir.path().join("base.txt"), "feature version\n").unwrap();
    git_commit(&dir, "modify base.txt on feature");

    // Main also modifies base.txt
    git_checkout(&dir, "main");
    std::fs::write(dir.path().join("base.txt"), "main version\n").unwrap();
    git_commit(&dir, "main: modify base.txt");
    git_checkout(&dir, "feature");
    let merge_hash = git_merge_with_conflict(&dir, "main");

    let result = get_review_file_system_data(
        &mut conn,
        &project_id,
        merge_hash,
        None,
        "commit".to_string(),
        &bc_id,
    )
    .unwrap();

    // base.txt has conflict changes → should NOT be merge_only
    let base_file = result.touched_files.iter().find(|f| f.path == "base.txt")
        .expect("base.txt should be in touched files");
    assert!(!base_file.merge_only, "conflict resolution files should not be merge_only");
}

#[test]
fn test_commit_single_no_merge_all_files_normal() {
    // Single commit review with no merges — all files should have merge_only: false
    let mut conn = test_conn();
    let (dir, base_hash) = setup_git_repo();
    let repo_path = dir.path().to_str().unwrap();
    let (project_id, bc_id) = setup_db_records(&mut conn, repo_path);

    // Review base commit
    create_event(&mut conn, &project_id, base_hash, "commit".to_string(), vec![], vec![], &bc_id).unwrap();

    std::fs::write(dir.path().join("new.txt"), "hello\n").unwrap();
    let c1 = git_commit(&dir, "add new.txt");

    let result = get_review_file_system_data(
        &mut conn,
        &project_id,
        c1,
        None,
        "commit".to_string(),
        &bc_id,
    )
    .unwrap();

    for f in &result.touched_files {
        assert!(!f.merge_only, "file {} should not be merge_only in single commit", f.path);
    }
}

#[test]
fn test_commit_non_base_merge_files_not_merge_only() {
    // Non-base merges (e.g. merging a feature branch into another feature branch)
    // should be treated as normal commits — their files should NOT be merge_only
    let mut conn = test_conn();
    let (dir, base_hash) = setup_git_repo();
    let repo_path = dir.path().to_str().unwrap();

    setup_feature_branch(&dir);
    let (project_id, bc_id) =
        setup_db_records_with_branch(&mut conn, repo_path, "feature", "main");

    // Create another branch "other" from the same base
    git_checkout(&dir, "main");
    Command::new("git")
        .args(["checkout", "-b", "other"])
        .current_dir(dir.path())
        .output()
        .unwrap();
    std::fs::write(dir.path().join("other.txt"), "other\n").unwrap();
    git_commit(&dir, "add other.txt on other");

    // Back to feature, merge "other" (not the base branch)
    git_checkout(&dir, "feature");
    let merge_hash = git_merge(&dir, "other");

    let result = get_review_file_system_data(
        &mut conn,
        &project_id,
        merge_hash,
        None,
        "commit".to_string(),
        &bc_id,
    )
    .unwrap();

    // other.txt comes from a non-base merge → should NOT be merge_only
    let other_file = result.touched_files.iter().find(|f| f.path == "other.txt")
        .expect("other.txt should be in touched files");
    assert!(!other_file.merge_only, "non-base-merge files should not be merge_only");
}

// ---------------------------------------------------------------------------
// Branch-mode get_review_file_system_data tests
// ---------------------------------------------------------------------------

#[test]
fn test_branch_file_system_data_added_modified_deleted() {
    let mut conn = test_conn();
    let (dir, _base_hash) = setup_git_repo();
    let repo_path = dir.path().to_str().unwrap();

    setup_feature_branch(&dir);
    let (project_id, bc_id) =
        setup_db_records_with_branch(&mut conn, repo_path, "feature", "main");

    // Commit 1: add a new file and modify base.txt
    std::fs::write(dir.path().join("base.txt"), "modified on feature\n").unwrap();
    std::fs::write(dir.path().join("feature_file.txt"), "new content\n").unwrap();
    let _c1 = git_commit(&dir, "add and modify on feature");

    // Commit 2: delete base.txt
    Command::new("git")
        .args(["rm", "base.txt"])
        .current_dir(dir.path())
        .output()
        .unwrap();
    let c2 = git_commit(&dir, "delete base.txt on feature");

    // Branch-mode diff is main..HEAD, so it accumulates all changes on the branch
    let result = get_review_file_system_data(
        &mut conn,
        &project_id,
        c2,
        None,
        "branch".to_string(),
        &bc_id,
    )
    .unwrap();

    // base.txt was modified then deleted — net result is D
    let base = result
        .touched_files
        .iter()
        .find(|f| f.path == "base.txt")
        .expect("base.txt should appear as deleted");
    assert_eq!(base.diff_mode.as_deref(), Some("D"));

    // feature_file.txt was added
    let added = result
        .touched_files
        .iter()
        .find(|f| f.path == "feature_file.txt")
        .expect("feature_file.txt should appear as added");
    assert_eq!(added.diff_mode.as_deref(), Some("A"));

    // All files should have state "green" (no issues filed)
    for f in &result.touched_files {
        assert_eq!(
            f.state, "green",
            "file {} should be green with no issues",
            f.path
        );
    }
}

/// Sets up a feature branch with two files (a.txt, b.txt) and issues on each.
/// Returns (conn, dir, project_id, bc_id, commit_hash, issue_a_id, issue_b_id).
fn setup_branch_two_file_issues() -> (
    SqliteConnection,
    TempDir,
    String,
    String,
    String,
    String,
    String,
) {
    let mut conn = test_conn();
    let (dir, _base_hash) = setup_git_repo();
    let repo_path = dir.path().to_str().unwrap().to_string();

    setup_feature_branch(&dir);
    let (project_id, bc_id) =
        setup_db_records_with_branch(&mut conn, &repo_path, "feature", "main");

    // Add two files on the feature branch
    std::fs::write(dir.path().join("a.txt"), "aaa\n").unwrap();
    std::fs::write(dir.path().join("b.txt"), "bbb\n").unwrap();
    let c1 = git_commit(&dir, "add a.txt and b.txt");

    // Review c1 with issues on both files
    create_event(
        &mut conn,
        &project_id,
        c1.clone(),
        "commit".to_string(),
        vec![
            NewIssueInput {
                comment: "problem in a".to_string(),
                reviews: vec![NewIssueReviewInput {
                    type_: "line".to_string(),
                    path: Some("a.txt".to_string()),
                    start: Some(1),
                    end: Some(1),
                    state: "red".to_string(),
                }],
            },
            NewIssueInput {
                comment: "problem in b".to_string(),
                reviews: vec![NewIssueReviewInput {
                    type_: "line".to_string(),
                    path: Some("b.txt".to_string()),
                    start: Some(1),
                    end: Some(1),
                    state: "yellow".to_string(),
                }],
            },
        ],
        vec![],
        &bc_id,
    )
    .unwrap();

    let all_issues = issue_repo::list(&mut conn, |q| q).unwrap();
    let issue_a = all_issues
        .iter()
        .find(|i| i.comment == "problem in a")
        .unwrap();
    let issue_b = all_issues
        .iter()
        .find(|i| i.comment == "problem in b")
        .unwrap();

    (
        conn,
        dir,
        project_id,
        bc_id,
        c1,
        issue_a.id.clone(),
        issue_b.id.clone(),
    )
}

#[test]
fn test_branch_file_system_data_no_issue_filter_both_red() {
    let (mut conn, _dir, project_id, bc_id, c1, _, _) = setup_branch_two_file_issues();

    let result = get_review_file_system_data(
        &mut conn,
        &project_id,
        c1,
        None,
        "branch".to_string(),
        &bc_id,
    )
    .unwrap();

    // Both files should be "red" (both have issue xrefs)
    let a = result
        .touched_files
        .iter()
        .find(|f| f.path == "a.txt")
        .expect("a.txt should be in touched files");
    assert_eq!(a.state, "red");

    let b = result
        .touched_files
        .iter()
        .find(|f| f.path == "b.txt")
        .expect("b.txt should be in touched files");
    assert_eq!(b.state, "red");
}

#[test]
fn test_branch_file_system_data_issue_filter_only_matching_file_red() {
    let (mut conn, _dir, project_id, bc_id, c1, issue_a_id, _) = setup_branch_two_file_issues();

    // Filter by issue_a — only a.txt's composite should be considered
    let result = get_review_file_system_data(
        &mut conn,
        &project_id,
        c1,
        Some(&issue_a_id),
        "branch".to_string(),
        &bc_id,
    )
    .unwrap();

    // a.txt should be "red" (matches the filtered issue)
    let a = result
        .touched_files
        .iter()
        .find(|f| f.path == "a.txt")
        .expect("a.txt should be in touched files");
    assert_eq!(a.state, "red");

    // b.txt should be "green" (its issue is filtered out)
    let b = result
        .touched_files
        .iter()
        .find(|f| f.path == "b.txt")
        .expect("b.txt should be in touched files");
    assert_eq!(b.state, "green");
}

// ---------------------------------------------------------------------------
// Branch-mode get_review_file_data tests
// ---------------------------------------------------------------------------

#[test]
fn test_branch_file_data_added_file() {
    let mut conn = test_conn();
    let (dir, _base_hash) = setup_git_repo();
    let repo_path = dir.path().to_str().unwrap();

    setup_feature_branch(&dir);
    let (project_id, bc_id) =
        setup_db_records_with_branch(&mut conn, repo_path, "feature", "main");

    std::fs::write(dir.path().join("new.txt"), "feature content\n").unwrap();
    let c1 = git_commit(&dir, "add new.txt on feature");

    let result = get_review_file_data(
        &mut conn,
        &project_id,
        c1,
        None,
        "branch".to_string(),
        "new.txt".to_string(),
        &bc_id,
    )
    .unwrap();

    assert_eq!(result.content, "feature content\n");
    // Diff is the file at main — doesn't exist, so empty
    assert_eq!(result.diff.as_deref(), Some(""));
}

#[test]
fn test_branch_file_data_modified_file() {
    let mut conn = test_conn();
    let (dir, _base_hash) = setup_git_repo();
    let repo_path = dir.path().to_str().unwrap();

    setup_feature_branch(&dir);
    let (project_id, bc_id) =
        setup_db_records_with_branch(&mut conn, repo_path, "feature", "main");

    std::fs::write(dir.path().join("base.txt"), "changed on feature\n").unwrap();
    let c1 = git_commit(&dir, "modify base.txt on feature");

    let result = get_review_file_data(
        &mut conn,
        &project_id,
        c1,
        None,
        "branch".to_string(),
        "base.txt".to_string(),
        &bc_id,
    )
    .unwrap();

    // Content is the feature version
    assert_eq!(result.content, "changed on feature\n");
    // Diff is the main version (original)
    assert_eq!(result.diff.as_deref(), Some("base\n"));
}

#[test]
fn test_branch_file_data_deleted_file() {
    let mut conn = test_conn();
    let (dir, _base_hash) = setup_git_repo();
    let repo_path = dir.path().to_str().unwrap();

    setup_feature_branch(&dir);
    let (project_id, bc_id) =
        setup_db_records_with_branch(&mut conn, repo_path, "feature", "main");

    Command::new("git")
        .args(["rm", "base.txt"])
        .current_dir(dir.path())
        .output()
        .unwrap();
    let c1 = git_commit(&dir, "delete base.txt on feature");

    let result = get_review_file_data(
        &mut conn,
        &project_id,
        c1,
        None,
        "branch".to_string(),
        "base.txt".to_string(),
        &bc_id,
    )
    .unwrap();

    // Content is empty (deleted at this commit)
    assert_eq!(result.content, "");
    // Diff is the main version
    assert_eq!(result.diff.as_deref(), Some("base\n"));
}

/// Sets up a feature branch with one file (code.txt) and two issues on different line ranges.
/// Returns (conn, dir, project_id, bc_id, commit_hash, issue_top_id, issue_bottom_id).
fn setup_branch_two_issues_same_file() -> (
    SqliteConnection,
    TempDir,
    String,
    String,
    String,
    String,
    String,
) {
    let mut conn = test_conn();
    let (dir, _base_hash) = setup_git_repo();
    let repo_path = dir.path().to_str().unwrap().to_string();

    setup_feature_branch(&dir);
    let (project_id, bc_id) =
        setup_db_records_with_branch(&mut conn, &repo_path, "feature", "main");

    std::fs::write(dir.path().join("code.txt"), "a\nb\nc\nd\ne\nf\n").unwrap();
    let c1 = git_commit(&dir, "add code.txt on feature");

    // Review c1 with two issues on different line ranges
    create_event(
        &mut conn,
        &project_id,
        c1.clone(),
        "commit".to_string(),
        vec![
            NewIssueInput {
                comment: "top issue".to_string(),
                reviews: vec![NewIssueReviewInput {
                    type_: "line".to_string(),
                    path: Some("code.txt".to_string()),
                    start: Some(1),
                    end: Some(2),
                    state: "red".to_string(),
                }],
            },
            NewIssueInput {
                comment: "bottom issue".to_string(),
                reviews: vec![NewIssueReviewInput {
                    type_: "line".to_string(),
                    path: Some("code.txt".to_string()),
                    start: Some(5),
                    end: Some(6),
                    state: "yellow".to_string(),
                }],
            },
        ],
        vec![],
        &bc_id,
    )
    .unwrap();

    let all_issues = issue_repo::list(&mut conn, |q| q).unwrap();
    let issue_top = all_issues
        .iter()
        .find(|i| i.comment == "top issue")
        .unwrap();
    let issue_bottom = all_issues
        .iter()
        .find(|i| i.comment == "bottom issue")
        .unwrap();

    (
        conn,
        dir,
        project_id,
        bc_id,
        c1,
        issue_top.id.clone(),
        issue_bottom.id.clone(),
    )
}

#[test]
fn test_branch_file_data_no_issue_filter_returns_all_line_summaries() {
    let (mut conn, _dir, project_id, bc_id, c1, _, _) = setup_branch_two_issues_same_file();

    let result = get_review_file_data(
        &mut conn,
        &project_id,
        c1,
        None,
        "branch".to_string(),
        "code.txt".to_string(),
        &bc_id,
    )
    .unwrap();

    assert_eq!(result.content, "a\nb\nc\nd\ne\nf\n");
    assert_eq!(result.line_summary.len(), 2);

    let starts: Vec<i32> = result.line_summary.iter().map(|l| l.start).collect();
    assert!(starts.contains(&1));
    assert!(starts.contains(&5));

    // Both issues should be listed
    assert_eq!(result.issues.len(), 2);
}

#[test]
fn test_branch_file_data_issue_filter_returns_only_matching_line_summary() {
    let (mut conn, _dir, project_id, bc_id, c1, issue_top_id, _) =
        setup_branch_two_issues_same_file();

    // Filter by the top issue
    let result = get_review_file_data(
        &mut conn,
        &project_id,
        c1,
        Some(&issue_top_id),
        "branch".to_string(),
        "code.txt".to_string(),
        &bc_id,
    )
    .unwrap();

    assert_eq!(result.content, "a\nb\nc\nd\ne\nf\n");

    // Only the top issue's line summary should be present
    assert_eq!(result.line_summary.len(), 1);
    assert_eq!(result.line_summary[0].start, 1);
    assert_eq!(result.line_summary[0].end, 2);
    assert_eq!(result.line_summary[0].state, "red");

    // issues field is NOT filtered by issue_id — it returns all issues for the file
    assert_eq!(result.issues.len(), 2);
}
