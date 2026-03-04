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
