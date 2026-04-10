use diesel::prelude::*;
use diesel::sqlite::SqliteConnection;
use diesel_migrations::MigrationHarness;
use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::{Arc, Mutex};
use tempfile::TempDir;

use crate::models::branch_context::NewBranchContext;
use crate::models::project::{NewProject, SshConfig};
use crate::repositories::{branch_context_repo, project_repo};
use crate::services::ssh_connection_manager::{SshError, SshExecutor};

// ---------------------------------------------------------------------------
// FakeSshExecutor
// ---------------------------------------------------------------------------

struct FakeEntry {
    stdout: Vec<u8>,
    stderr: Vec<u8>,
    status: i32,
}

/// Test double for `SshExecutor`. Enqueue canned responses keyed by a
/// command-prefix string. The first enqueued response whose key is a prefix
/// of the space-joined args string is consumed on each `run_command` call.
pub struct FakeSshExecutor {
    connect_result: Arc<Mutex<Result<(), String>>>,
    connected: Arc<Mutex<HashMap<String, bool>>>,
    responses: Arc<Mutex<Vec<(String, VecDeque<FakeEntry>)>>>,
}

fn make_output(entry: FakeEntry) -> Output {
    use std::os::unix::process::ExitStatusExt;
    Output {
        status: std::process::ExitStatus::from_raw(entry.status << 8),
        stdout: entry.stdout,
        stderr: entry.stderr,
    }
}

impl FakeSshExecutor {
    pub fn new() -> Self {
        Self {
            connect_result: Arc::new(Mutex::new(Ok(()))),
            connected: Arc::new(Mutex::new(HashMap::new())),
            responses: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Make the next `ensure_connected` call return the given result.
    pub fn set_connect_result(&mut self, result: Result<(), SshError>) {
        *self.connect_result.lock().unwrap() =
            result.map_err(|e| e.to_string());
    }

    /// Enqueue a response for the next `run_command` call whose args (joined
    /// by spaces) start with `prefix`.
    pub fn expect_command(&self, prefix: &str, stdout: &[u8], stderr: &[u8], status: i32) {
        let entry = FakeEntry {
            stdout: stdout.to_vec(),
            stderr: stderr.to_vec(),
            status,
        };
        let mut responses = self.responses.lock().unwrap();
        if let Some(slot) = responses.iter_mut().find(|(k, _)| k == prefix) {
            slot.1.push_back(entry);
        } else {
            let mut deque = VecDeque::new();
            deque.push_back(entry);
            responses.push((prefix.to_string(), deque));
        }
    }

    /// Simulate a mid-session ControlMaster death for a specific host.
    pub fn drop_connection(&self, config: &SshConfig) {
        self.connected
            .lock()
            .unwrap()
            .insert(config.connection_key(), false);
    }

    /// Panic if any enqueued response was never consumed.
    pub fn assert_all_expectations_met(&self) {
        let responses = self.responses.lock().unwrap();
        for (key, deque) in responses.iter() {
            assert!(
                deque.is_empty(),
                "Unused fake SSH response for prefix '{}'",
                key
            );
        }
    }
}

impl SshExecutor for FakeSshExecutor {
    fn ensure_connected(&self, config: &SshConfig) -> Result<(), SshError> {
        let result = self.connect_result.lock().unwrap();
        match &*result {
            Ok(()) => {
                self.connected
                    .lock()
                    .unwrap()
                    .insert(config.connection_key(), true);
                Ok(())
            }
            Err(msg) => Err(SshError::CommandFailed(msg.clone())),
        }
    }

    fn run_command(&self, _config: &SshConfig, args: &[&str]) -> Result<Output, SshError> {
        let joined = args.join(" ");
        let mut responses = self.responses.lock().unwrap();
        for (prefix, deque) in responses.iter_mut() {
            if joined.starts_with(prefix.as_str()) {
                if let Some(entry) = deque.pop_front() {
                    return Ok(make_output(entry));
                }
            }
        }
        // Default: success with empty output if no expectation matched.
        Ok(make_output(FakeEntry {
            stdout: vec![],
            stderr: vec![],
            status: 0,
        }))
    }

    fn is_connected(&self, config: &SshConfig) -> bool {
        self.connected
            .lock()
            .unwrap()
            .get(&config.connection_key())
            .copied()
            .unwrap_or(false)
    }

    fn socket_path(&self, config: &SshConfig) -> PathBuf {
        // Return a deterministic fake path for tests.
        std::env::temp_dir()
            .join("proveall-ssh-test")
            .join(format!("{}.ctl", config.connection_key().replace(['@', ':'], "_")))
    }

    fn disconnect(&self, config: &SshConfig) {
        self.connected
            .lock()
            .unwrap()
            .insert(config.connection_key(), false);
    }
}

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

/// Merge the given branch into the current branch (no fast-forward).
/// Returns the merge commit hash.
pub fn git_merge(dir: &TempDir, branch: &str) -> String {
    Command::new("git")
        .args([
            "merge",
            "--no-ff",
            branch,
            "-m",
            &format!("Merge {} into HEAD", branch),
        ])
        .current_dir(dir.path())
        .output()
        .unwrap();
    git_rev_parse(dir)
}

/// Switch to an existing branch.
pub fn git_checkout(dir: &TempDir, branch: &str) {
    Command::new("git")
        .args(["checkout", branch])
        .current_dir(dir.path())
        .output()
        .unwrap();
}

/// Merge a branch that has a conflict on `base.txt`, resolve it, and commit.
/// Assumes `base.txt` differs between current branch and `branch`.
/// Returns the merge commit hash.
pub fn git_merge_with_conflict(dir: &TempDir, branch: &str) -> String {
    let output = Command::new("git")
        .args([
            "merge",
            "--no-ff",
            branch,
            "-m",
            &format!("Merge {} (conflict)", branch),
        ])
        .current_dir(dir.path())
        .output()
        .unwrap();

    // If the merge succeeded without conflict, panic — caller set up wrong
    if output.status.success() {
        panic!("Expected merge conflict but merge succeeded cleanly");
    }

    // Resolve conflict by picking a combined version
    std::fs::write(dir.path().join("base.txt"), "resolved content\n").unwrap();
    Command::new("git")
        .args(["add", "base.txt"])
        .current_dir(dir.path())
        .output()
        .unwrap();
    Command::new("git")
        .args(["commit", "--no-edit"])
        .current_dir(dir.path())
        .output()
        .unwrap();
    git_rev_parse(dir)
}
