/// Remote git operations for SSH projects.
///
/// Each function dispatches to the local `git` binary (via `utils::git`) for
/// `connection_type = "local"` projects and to the SSH executor for
/// `connection_type = "ssh"` projects.
///
/// Local projects are completely unaffected; the SSH paths are exercised only
/// by tests (using `FakeSshExecutor`) until Commit 5 wires them into the app.
use crate::error::AppError;
use crate::models::project::Project;
use crate::services::ssh_connection_manager::{SshError, SshExecutor};
use crate::utils::git::{self, DiffFile, GitCommit};

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

fn ssh_error_to_app(e: SshError) -> AppError {
    AppError::Git(e.to_string())
}

/// Run a git command on a remote project via SSH and return stdout as a String.
fn run_remote_git(
    project: &Project,
    executor: &dyn SshExecutor,
    git_args: &[&str],
) -> Result<String, AppError> {
    let config = project
        .parse_ssh_config()
        .ok_or_else(|| AppError::Git("SSH project has no ssh_config".to_string()))?;

    // Build: git -C <remote_path> <git_args…>
    let remote_path = config.remote_path.clone();
    let mut full_args = vec!["git", "-C", &remote_path];
    full_args.extend_from_slice(git_args);

    let output = executor
        .run_command(&config, &full_args)
        .map_err(ssh_error_to_app)?;

    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();

    if output.status.success() {
        Ok(stdout)
    } else {
        Err(AppError::Git(stderr))
    }
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

pub fn current_branch(project: &Project, executor: &dyn SshExecutor) -> Result<String, AppError> {
    if !project.is_ssh() {
        return git::current_branch(&project.path);
    }
    let stdout = run_remote_git(project, executor, &["rev-parse", "--abbrev-ref", "HEAD"])?;
    Ok(stdout.trim().to_string())
}

pub fn rev_parse(
    project: &Project,
    executor: &dyn SshExecutor,
    git_ref: &str,
) -> Result<String, AppError> {
    if !project.is_ssh() {
        return git::rev_parse(&project.path, git_ref);
    }
    let stdout = run_remote_git(project, executor, &["rev-parse", git_ref])?;
    Ok(stdout.trim().to_string())
}

pub fn diff_shortstat(
    project: &Project,
    executor: &dyn SshExecutor,
    range: &str,
) -> Result<String, AppError> {
    if !project.is_ssh() {
        return git::diff_shortstat(&project.path, range);
    }
    run_remote_git(project, executor, &["diff", "--shortstat", range])
}

pub fn log(
    project: &Project,
    executor: &dyn SshExecutor,
    args: &[&str],
) -> Result<Vec<GitCommit>, AppError> {
    if !project.is_ssh() {
        return git::log(&project.path, args);
    }
    let mut cmd_args = vec!["log", "--format=%H%n%P%n%s%n%an%n%aI"];
    cmd_args.extend_from_slice(args);
    let stdout = run_remote_git(project, executor, &cmd_args)?;
    let lines: Vec<&str> = stdout.lines().collect();
    Ok(lines
        .chunks(5)
        .map(|chunk| GitCommit {
            hash: chunk[0].to_string(),
            parents: chunk[1]
                .split_whitespace()
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string())
                .collect(),
            subject: chunk[2].to_string(),
            author: chunk[3].to_string(),
            date: chunk[4].to_string(),
        })
        .collect())
}

pub fn show(
    project: &Project,
    executor: &dyn SshExecutor,
    git_ref: &str,
) -> Result<String, AppError> {
    if !project.is_ssh() {
        return git::show(&project.path, git_ref);
    }
    run_remote_git(project, executor, &["show", git_ref])
}

pub fn diff_file(
    project: &Project,
    executor: &dyn SshExecutor,
    prev: &str,
    curr: &str,
    file_path: &str,
) -> Result<String, AppError> {
    if !project.is_ssh() {
        return git::diff_file(&project.path, prev, curr, file_path);
    }
    run_remote_git(
        project,
        executor,
        &["diff", prev, curr, "--", file_path],
    )
}

pub fn diff_changed_files(
    project: &Project,
    executor: &dyn SshExecutor,
    range_args: &[&str],
) -> Result<Vec<DiffFile>, AppError> {
    if !project.is_ssh() {
        return git::diff_changed_files(&project.path, range_args);
    }
    let mut args = vec!["diff", "--name-status"];
    args.extend_from_slice(range_args);
    let stdout = run_remote_git(project, executor, &args)?;

    // Reuse the same parsing logic from git.rs (via re-exposed helper).
    Ok(stdout
        .lines()
        .filter(|l| !l.is_empty())
        .filter_map(|line| {
            let mut parts = line.splitn(2, '\t');
            let status = parts.next()?.chars().next()?.to_string();
            let path = parts.next()?.to_string();
            Some(DiffFile { status, path })
        })
        .collect())
}

pub fn is_ancestor(
    project: &Project,
    executor: &dyn SshExecutor,
    ancestor: &str,
    descendant: &str,
) -> bool {
    if !project.is_ssh() {
        return git::is_ancestor(&project.path, ancestor, descendant);
    }
    run_remote_git(
        project,
        executor,
        &["merge-base", "--is-ancestor", ancestor, descendant],
    )
    .is_ok()
}

pub fn is_base_branch_merge(
    project: &Project,
    executor: &dyn SshExecutor,
    parents: &[String],
    base_branch: &str,
) -> bool {
    let base_hash = match rev_parse(project, executor, base_branch) {
        Ok(h) => h,
        Err(_) => return false,
    };
    parents
        .iter()
        .any(|p| is_ancestor(project, executor, p, &base_hash))
}

pub fn diff_tree_cc(
    project: &Project,
    executor: &dyn SshExecutor,
    hash: &str,
) -> Result<String, AppError> {
    if !project.is_ssh() {
        return git::diff_tree_cc(&project.path, hash);
    }
    let stdout = run_remote_git(project, executor, &["diff-tree", "--cc", "-p", hash])?;
    Ok(stdout.lines().skip(1).collect::<Vec<_>>().join("\n"))
}

pub fn diff_tree_cc_name_only(
    project: &Project,
    executor: &dyn SshExecutor,
    hash: &str,
) -> Result<String, AppError> {
    if !project.is_ssh() {
        return git::diff_tree_cc_name_only(&project.path, hash);
    }
    let stdout = run_remote_git(project, executor, &["diff-tree", "--cc", "--name-only", hash])?;
    Ok(stdout.lines().skip(1).collect::<Vec<_>>().join("\n"))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::project::{NewProject, SshConfig};
    use crate::test_utils::FakeSshExecutor;

    fn ssh_project() -> Project {
        let config = SshConfig {
            host: "testhost.example.com".to_string(),
            user: "alice".to_string(),
            port: 22,
            key_path: None,
            remote_path: "/home/alice/myapp".to_string(),
        };
        let new = NewProject::new_ssh(&config);
        Project {
            id: new.id,
            path: new.path,
            name: new.name,
            created_at: chrono::NaiveDateTime::default(),
            updated_at: chrono::NaiveDateTime::default(),
            last_opened: None,
            connection_type: "ssh".to_string(),
            ssh_config: new.ssh_config,
        }
    }

    #[test]
    fn test_current_branch_remote() {
        let project = ssh_project();
        let fake = FakeSshExecutor::new();
        fake.expect_command("git -C /home/alice/myapp rev-parse", b"main\n", b"", 0);
        fake.ensure_connected(&project.parse_ssh_config().unwrap())
            .unwrap();
        let branch = current_branch(&project, &fake).unwrap();
        assert_eq!(branch, "main");
    }

    #[test]
    fn test_current_branch_remote_trims_whitespace() {
        let project = ssh_project();
        let fake = FakeSshExecutor::new();
        fake.expect_command(
            "git -C /home/alice/myapp rev-parse",
            b"  feature-branch  \n",
            b"",
            0,
        );
        fake.ensure_connected(&project.parse_ssh_config().unwrap())
            .unwrap();
        let branch = current_branch(&project, &fake).unwrap();
        assert_eq!(branch, "feature-branch");
    }

    #[test]
    fn test_git_log_remote() {
        let project = ssh_project();
        let fake = FakeSshExecutor::new();
        // 5-line format: hash, parents, subject, author, date
        let log_output =
            b"abc123\ndef456\nFix bug\nAlice Smith\n2024-01-01T00:00:00+00:00\n";
        fake.expect_command("git -C /home/alice/myapp log", log_output, b"", 0);
        fake.ensure_connected(&project.parse_ssh_config().unwrap())
            .unwrap();
        let commits = log(&project, &fake, &["--no-merges"]).unwrap();
        assert_eq!(commits.len(), 1);
        assert_eq!(commits[0].hash, "abc123");
        assert_eq!(commits[0].subject, "Fix bug");
        assert_eq!(commits[0].author, "Alice Smith");
    }

    #[test]
    fn test_git_command_failure() {
        let project = ssh_project();
        let fake = FakeSshExecutor::new();
        fake.expect_command(
            "git -C /home/alice/myapp rev-parse",
            b"",
            b"not a git repository",
            128,
        );
        fake.ensure_connected(&project.parse_ssh_config().unwrap())
            .unwrap();
        let result = current_branch(&project, &fake);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not a git repository"));
    }

    #[test]
    fn test_diff_changed_files_remote() {
        let project = ssh_project();
        let fake = FakeSshExecutor::new();
        let output = b"M\tsrc/main.rs\nA\tsrc/lib.rs\nD\told.txt\n";
        fake.expect_command("git -C /home/alice/myapp diff", output, b"", 0);
        fake.ensure_connected(&project.parse_ssh_config().unwrap())
            .unwrap();
        let files = diff_changed_files(&project, &fake, &["HEAD~1..HEAD"]).unwrap();
        assert_eq!(files.len(), 3);
        assert_eq!(files[0].status, "M");
        assert_eq!(files[0].path, "src/main.rs");
        assert_eq!(files[2].status, "D");
    }

    #[test]
    fn test_diff_file_remote() {
        let project = ssh_project();
        let fake = FakeSshExecutor::new();
        let diff = b"--- a/file.rs\n+++ b/file.rs\n@@ -1 +1 @@\n-old\n+new\n";
        fake.expect_command("git -C /home/alice/myapp diff", diff, b"", 0);
        fake.ensure_connected(&project.parse_ssh_config().unwrap())
            .unwrap();
        let result = diff_file(&project, &fake, "abc", "def", "file.rs").unwrap();
        assert!(result.contains("--- a/file.rs"));
    }

    #[test]
    fn test_git_args_forwarded_correctly() {
        // Verify that -C <remote_path> is prepended to git args for SSH projects.
        let project = ssh_project();
        let fake = FakeSshExecutor::new();
        // rev_parse with a specific ref
        fake.expect_command("git -C /home/alice/myapp rev-parse", b"deadbeef\n", b"", 0);
        fake.ensure_connected(&project.parse_ssh_config().unwrap())
            .unwrap();
        let hash = rev_parse(&project, &fake, "HEAD").unwrap();
        assert_eq!(hash, "deadbeef");
        fake.assert_all_expectations_met();
    }
}
