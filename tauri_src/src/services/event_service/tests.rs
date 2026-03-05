use super::*;
use crate::commands::event_commands::{NewIssueInput, NewIssueReviewInput};
use crate::models::branch_context::NewBranchContext;
use crate::models::composite_file_review_state::ReviewSummaryMetadataEntry;
use crate::models::project::NewProject;
use diesel_migrations::MigrationHarness;
use std::process::Command;
use tempfile::TempDir;

/// Create an in-memory SQLite connection with all migrations applied.
fn test_conn() -> SqliteConnection {
    let mut conn =
        SqliteConnection::establish(":memory:").expect("Failed to create in-memory connection");
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
    create_event(
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
    let c1_xrefs =
        event_issue_composite_xref_repo::join_list_by_event(&mut conn, &c1_result.id, &bc_id)
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
    let c2_xrefs =
        event_issue_composite_xref_repo::join_list_by_event(&mut conn, &c2_event.id, &bc_id)
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
    assert_eq!(c3_xrefs.len(), 1, "Expected 1 xref propagated to c3 target");

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
    let c1_xrefs =
        event_issue_composite_xref_repo::join_list_by_event(&mut conn, &c1_event.id, &bc_id)
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

#[test]
fn test_first_review_on_branch_with_multiple_commits_creates_intermediates() {
    let mut conn = test_conn();
    let (dir, _base_hash) = setup_git_repo();
    let repo_path = dir.path().to_str().unwrap();

    // Create a feature branch off main
    Command::new("git")
        .args(["checkout", "-b", "feature"])
        .current_dir(dir.path())
        .output()
        .unwrap();

    // Create 3 commits on the feature branch
    let c1 = add_commit(&dir, "file.txt", "line1\nline2\n", "feature commit 1");
    let c2 = add_commit(
        &dir,
        "file.txt",
        "line1\nline2\nline3\n",
        "feature commit 2",
    );
    let c3 = add_commit(
        &dir,
        "file.txt",
        "line1\nline2\nline3\nline4\n",
        "feature commit 3",
    );

    // Set up project and branch context with base_branch = main
    let project = project_repo::create(
        &mut conn,
        NewProject::new(repo_path.to_string(), Some("test".to_string())),
    )
    .expect("Failed to create project");

    let bc = branch_context_repo::create(
        &mut conn,
        NewBranchContext::new(
            project.id.clone(),
            "feature".to_string(),
            "main".to_string(),
            "{}".to_string(),
        ),
    )
    .expect("Failed to create branch context");

    // First review ever on this branch — target c3 directly (no previous event)
    // This should create intermediate events for c1 and c2
    let result = create_event(
        &mut conn,
        &project.id,
        c3.clone(),
        "commit".to_string(),
        vec![],
        vec![],
        &bc.id,
    )
    .expect("Failed to create first bulk event on branch");

    // Should have 3 events: c1 (intermediate) + c2 (intermediate) + c3 (target)
    let all_events = event_repo::list(&mut conn, |q| q).expect("Failed to list events");
    assert_eq!(
        all_events.len(),
        3,
        "Expected 3 events (2 intermediates + target)"
    );

    // Verify each commit has an event
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

    let c3_events: Vec<_> = all_events
        .iter()
        .filter(|e| e.hash.as_deref() == Some(&c3))
        .collect();
    assert_eq!(c3_events.len(), 1, "Expected exactly 1 event for c3");

    // head_event_id should point to the target (c3) event
    let bc_updated = branch_context_repo::get(&mut conn, &bc.id).unwrap();
    assert_eq!(bc_updated.head_event_id, Some(result.id));
}

#[test]
fn test_resolve_one_issue_then_bulk_review_with_translated_lines() {
    let mut conn = test_conn();
    let (dir, base_hash) = setup_git_repo();
    let repo_path = dir.path().to_str().unwrap();
    let (project_id, bc_id) = setup_db_records(&mut conn, repo_path);

    // --- Step 1: commit c1 that adds content to file.txt and a second file ---
    // file.txt gets 10 lines; issue A will be on lines 8-10 (far from where c2 edits)
    std::fs::write(
        dir.path().join("file.txt"),
        "line1\nline2\nline3\nline4\nline5\nline6\nline7\nline8\nline9\nline10\n",
    )
    .unwrap();
    std::fs::write(dir.path().join("other.txt"), "a\nb\nc\n").unwrap();
    Command::new("git")
        .args(["add", "."])
        .current_dir(dir.path())
        .output()
        .unwrap();
    Command::new("git")
        .args([
            "commit",
            "-m",
            "commit 1 - expand file.txt and add other.txt",
        ])
        .current_dir(dir.path())
        .output()
        .unwrap();
    let c1 = {
        let output = Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(dir.path())
            .output()
            .unwrap();
        String::from_utf8_lossy(&output.stdout).trim().to_string()
    };

    // Review base commit
    let _base_event = create_event(
        &mut conn,
        &project_id,
        base_hash.clone(),
        "commit".to_string(),
        vec![],
        vec![],
        &bc_id,
    )
    .expect("Failed to create base event");

    // --- Step 2: review c1 with 2 issues ---
    // Issue A: file.txt lines 8-10 (red) — near the bottom, away from where c2 edits
    // Issue B: other.txt lines 1-3 (yellow)
    let c1_event = create_event(
        &mut conn,
        &project_id,
        c1.clone(),
        "commit".to_string(),
        vec![
            NewIssueInput {
                comment: "issue A on file.txt".to_string(),
                reviews: vec![NewIssueReviewInput {
                    type_: "line".to_string(),
                    path: Some("file.txt".to_string()),
                    start: Some(8),
                    end: Some(10),
                    state: "red".to_string(),
                }],
            },
            NewIssueInput {
                comment: "issue B on other.txt".to_string(),
                reviews: vec![NewIssueReviewInput {
                    type_: "line".to_string(),
                    path: Some("other.txt".to_string()),
                    start: Some(1),
                    end: Some(3),
                    state: "yellow".to_string(),
                }],
            },
        ],
        vec![],
        &bc_id,
    )
    .expect("Failed to create c1 event with issues");

    // Identify the two issue IDs
    let all_issues = issue_repo::list(&mut conn, |q| q).expect("list issues");
    assert_eq!(all_issues.len(), 2);
    let issue_a = all_issues
        .iter()
        .find(|i| i.comment == "issue A on file.txt")
        .unwrap();
    let issue_b = all_issues
        .iter()
        .find(|i| i.comment == "issue B on other.txt")
        .unwrap();

    // Verify c1 has 2 xrefs (one per issue)
    let c1_xrefs =
        event_issue_composite_xref_repo::join_list_by_event(&mut conn, &c1_event.id, &bc_id)
            .expect("c1 xrefs");
    assert_eq!(c1_xrefs.len(), 2, "c1 should have 2 xrefs");

    // --- Step 3: resolve issue B ---
    let resolution_event = create_event(
        &mut conn,
        &project_id,
        c1.clone(),
        "resolution".to_string(),
        vec![],
        vec![issue_b.id.clone()],
        &bc_id,
    )
    .expect("Failed to create resolution event");

    // Resolution event should only propagate issue A's xref (issue B is resolved)
    let res_xrefs = event_issue_composite_xref_repo::join_list_by_event(
        &mut conn,
        &resolution_event.id,
        &bc_id,
    )
    .expect("resolution xrefs");
    assert_eq!(
        res_xrefs.len(),
        1,
        "Resolution event should have 1 xref (only issue A)"
    );
    assert_eq!(res_xrefs[0].0.issue_id, issue_a.id);

    // --- Step 4: two more commits that touch file.txt (shifting lines) ---
    // c2: insert 2 lines near the top of file.txt (between line1 and line2),
    //     pushing issue A's lines (8-10) down by 2 to (10-12)
    let c2 = add_commit(
        &dir,
        "file.txt",
        "line1\ninserted1\ninserted2\nline2\nline3\nline4\nline5\nline6\nline7\nline8\nline9\nline10\n",
        "commit 2 - insert near top of file.txt",
    );

    // c3: append to file.txt (after line 12, shouldn't affect issue A's translated lines)
    let c3 = add_commit(
        &dir,
        "file.txt",
        "line1\ninserted1\ninserted2\nline2\nline3\nline4\nline5\nline6\nline7\nline8\nline9\nline10\nline11\n",
        "commit 3 - append to file.txt",
    );

    // --- Step 5: bulk review c3 (skipping c2 — creates intermediate for c2) ---
    let c3_event = create_event(
        &mut conn,
        &project_id,
        c3.clone(),
        "commit".to_string(),
        vec![],
        vec![],
        &bc_id,
    )
    .expect("Failed to create bulk event for c3");

    // --- Validate event count and order ---
    // Expected events: base, c1, resolution, c2 (intermediate), c3 (target) = 5
    let all_events = event_repo::list(&mut conn, |q| q).expect("list events");
    assert_eq!(all_events.len(), 5, "Expected 5 events total");

    // Verify events exist with correct hashes
    let c2_event = all_events
        .iter()
        .find(|e| e.hash.as_deref() == Some(c2.as_str()))
        .expect("Expected an event for c2");
    let c3_db_event = all_events
        .iter()
        .find(|e| e.hash.as_deref() == Some(c3.as_str()))
        .expect("Expected an event for c3");

    // Verify chronological order: base < c1 < resolution < c2 < c3
    let base_event = all_events
        .iter()
        .find(|e| e.hash.as_deref() == Some(base_hash.as_str()))
        .unwrap();
    let c1_db_event = all_events
        .iter()
        .find(|e| e.hash.as_deref() == Some(c1.as_str()))
        .unwrap();
    let res_db_event = all_events.iter().find(|e| e.type_ == "resolution").unwrap();

    assert!(base_event.created_at <= c1_db_event.created_at);
    assert!(c1_db_event.created_at <= res_db_event.created_at);
    assert!(res_db_event.created_at <= c2_event.created_at);
    assert!(c2_event.created_at <= c3_db_event.created_at);

    // --- Validate xrefs: only issue A should propagate (issue B was resolved) ---

    // c2 intermediate should have 1 xref for issue A only
    let c2_xrefs =
        event_issue_composite_xref_repo::join_list_by_event(&mut conn, &c2_event.id, &bc_id)
            .expect("c2 xrefs");
    assert_eq!(c2_xrefs.len(), 1, "c2 should have 1 xref (issue A only)");
    assert_eq!(c2_xrefs[0].0.issue_id, issue_a.id);

    // c3 target should have 1 xref for issue A only
    let c3_xrefs =
        event_issue_composite_xref_repo::join_list_by_event(&mut conn, &c3_event.id, &bc_id)
            .expect("c3 xrefs");
    assert_eq!(c3_xrefs.len(), 1, "c3 should have 1 xref (issue A only)");
    assert_eq!(c3_xrefs[0].0.issue_id, issue_a.id);

    // No xrefs should reference issue B anywhere after the resolution
    assert_ne!(c2_xrefs[0].0.issue_id, issue_b.id);
    assert_ne!(c3_xrefs[0].0.issue_id, issue_b.id);

    // --- Validate composite line translation for issue A ---
    // Original issue A was on file.txt lines 8-10.
    // c2 inserted 2 lines near the top (after line 1), so lines 8-10 shift to 10-12.
    // c3 only appended at the bottom (after line 12), so lines remain 10-12.

    // c2's composite for issue A should have translated lines
    let c2_composite = &c2_xrefs[0].1;
    assert_eq!(c2_composite.relative_file_path, "file.txt");
    let c2_entries: Vec<ReviewSummaryMetadataEntry> =
        serde_json::from_str(&c2_composite.summary_metadata).expect("parse c2 metadata");
    assert_eq!(c2_entries.len(), 1);
    assert_eq!(
        c2_entries[0].start, 10,
        "c2: issue A start should shift from 8 to 10"
    );
    assert_eq!(
        c2_entries[0].end, 12,
        "c2: issue A end should shift from 10 to 12"
    );
    assert_eq!(c2_entries[0].state, "red");

    // c3's composite should keep the same translated lines (append doesn't affect them)
    let c3_composite = &c3_xrefs[0].1;
    assert_eq!(c3_composite.relative_file_path, "file.txt");
    let c3_entries: Vec<ReviewSummaryMetadataEntry> =
        serde_json::from_str(&c3_composite.summary_metadata).expect("parse c3 metadata");
    assert_eq!(c3_entries.len(), 1);
    assert_eq!(
        c3_entries[0].start, 10,
        "c3: issue A start should remain at 10"
    );
    assert_eq!(c3_entries[0].end, 12, "c3: issue A end should remain at 12");
    assert_eq!(c3_entries[0].state, "red");

    // head_event_id should point to c3
    let bc = branch_context_repo::get(&mut conn, &bc_id).unwrap();
    assert_eq!(bc.head_event_id, Some(c3_event.id));
}
