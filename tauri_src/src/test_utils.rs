use diesel::prelude::*;
use diesel::sqlite::SqliteConnection;
use diesel_migrations::MigrationHarness;
use std::process::Command;
use tempfile::TempDir;

use crate::models::branch_context::NewBranchContext;
use crate::models::project::NewProject;
use crate::repositories::{branch_context_repo, project_repo};

/// Create an in-memory SQLite connection with all migrations applied.
pub fn test_conn() -> SqliteConnection {
    let mut conn =
        SqliteConnection::establish(":memory:").expect("Failed to create in-memory connection");
    diesel::sql_query("PRAGMA foreign_keys = ON")
        .execute(&mut conn)
        .ok();
    conn.run_pending_migrations(crate::db::connection::MIGRATIONS)
        .expect("Failed to run migrations");
    conn
}

/// Create a temporary git repo with an initial commit containing `base.txt`.
/// Returns the TempDir and the initial commit hash.
pub fn setup_git_repo() -> (TempDir, String) {
    let dir = TempDir::new().expect("Failed to create temp dir");
    let path = dir.path();

    let git = |args: &[&str]| {
        Command::new("git")
            .args(args)
            .current_dir(path)
            .output()
            .expect("git command failed")
    };

    git(&["init", "-b", "main"]);
    git(&["config", "user.email", "test@test.com"]);
    git(&["config", "user.name", "Test"]);

    std::fs::write(path.join("base.txt"), "base\n").unwrap();
    git(&["add", "."]);
    git(&["commit", "-m", "initial"]);

    let output = git(&["rev-parse", "HEAD"]);
    let base_hash = String::from_utf8_lossy(&output.stdout).trim().to_string();

    (dir, base_hash)
}

/// Get the current HEAD commit hash.
pub fn git_rev_parse(dir: &TempDir) -> String {
    let output = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(dir.path())
        .output()
        .unwrap();
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

/// Stage all changes and commit with the given message. Returns the new commit hash.
pub fn git_commit(dir: &TempDir, message: &str) -> String {
    Command::new("git")
        .args(["add", "."])
        .current_dir(dir.path())
        .output()
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", message])
        .current_dir(dir.path())
        .output()
        .unwrap();
    git_rev_parse(dir)
}

/// Create a project and branch context in the DB with branch == base_branch == "main".
/// Returns (project_id, branch_context_id).
pub fn setup_db_records(conn: &mut SqliteConnection, repo_path: &str) -> (String, String) {
    setup_db_records_with_branch(conn, repo_path, "main", "main")
}

/// Create a project and branch context in the DB with the given branch and base_branch.
/// Returns (project_id, branch_context_id).
pub fn setup_db_records_with_branch(
    conn: &mut SqliteConnection,
    repo_path: &str,
    branch: &str,
    base_branch: &str,
) -> (String, String) {
    let project = project_repo::create(
        conn,
        NewProject::new(repo_path.to_string(), Some("test".to_string())),
    )
    .expect("Failed to create project");

    let bc = branch_context_repo::create(
        conn,
        NewBranchContext::new(
            project.id.clone(),
            branch.to_string(),
            base_branch.to_string(),
            "{}".to_string(),
        ),
    )
    .expect("Failed to create branch context");

    (project.id, bc.id)
}

/// Checkout a new "feature" branch from the current HEAD.
pub fn setup_feature_branch(dir: &TempDir) {
    Command::new("git")
        .args(["checkout", "-b", "feature"])
        .current_dir(dir.path())
        .output()
        .unwrap();
}
