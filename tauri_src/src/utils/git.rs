use std::process::Command;

use serde::Serialize;

use crate::error::AppError;

struct GitOutput {
    stdout: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiffFile {
    pub(crate) status: String,
    pub(crate) path: String,
}

fn run_git(project_path: &str, args: &[&str]) -> Result<GitOutput, AppError> {
    let output = Command::new("git")
        .args(args)
        .current_dir(project_path)
        .output()?;

    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();

    if output.status.success() {
        Ok(GitOutput { stdout })
    } else {
        Err(AppError::Git(stderr))
    }
}

// ---------------------------------------------------------------------------
// Named git commands
// ---------------------------------------------------------------------------

/// `git rev-parse --abbrev-ref HEAD` — current branch name.
pub fn current_branch(project_path: &str) -> Result<String, AppError> {
    Ok(run_git(project_path, &["rev-parse", "--abbrev-ref", "HEAD"])?
        .stdout
        .trim()
        .to_string())
}

/// `git rev-parse <git_ref>` — resolve a ref to a commit hash.
pub fn rev_parse(project_path: &str, git_ref: &str) -> Result<String, AppError> {
    Ok(run_git(project_path, &["rev-parse", git_ref])?
        .stdout
        .trim()
        .to_string())
}

/// `git diff --shortstat <range>` — raw shortstat output.
pub fn diff_shortstat(project_path: &str, range: &str) -> Result<String, AppError> {
    Ok(run_git(project_path, &["diff", "--shortstat", range])?
        .stdout)
}

pub struct GitCommit {
    pub hash: String,
    pub subject: String,
    pub author: String,
    pub date: String,
}

/// Structured git log. Extra args (e.g. "--no-merges", range, "-1 <hash>")
/// are appended after the format flag.
pub fn log(project_path: &str, args: &[&str]) -> Result<Vec<GitCommit>, AppError> {
    let mut cmd_args = vec!["log", "--format=%H%n%s%n%an%n%aI"];
    cmd_args.extend_from_slice(args);
    let output = run_git(project_path, &cmd_args)?;
    let lines: Vec<&str> = output.stdout.lines().collect();
    Ok(lines
        .chunks(4)
        .filter(|chunk| chunk.len() == 4)
        .map(|chunk| GitCommit {
            hash: chunk[0].to_string(),
            subject: chunk[1].to_string(),
            author: chunk[2].to_string(),
            date: chunk[3].to_string(),
        })
        .collect())
}

/// `git show <git_ref>` — raw object/file content.
pub fn show(project_path: &str, git_ref: &str) -> Result<String, AppError> {
    Ok(run_git(project_path, &["show", git_ref])?.stdout)
}

/// `git diff <prev_commit> <curr_commit> -- <file_path>` — unified diff for a single file.
pub fn diff_file(
    project_path: &str,
    prev: &str,
    curr: &str,
    file_path: &str,
) -> Result<String, AppError> {
    Ok(run_git(project_path, &["diff", prev, curr, "--", file_path])?
        .stdout)
}

/// `git add -A` — stage all changes.
pub fn add_all(project_path: &str) -> Result<(), AppError> {
    run_git(project_path, &["add", "-A"])?;
    Ok(())
}

/// `git status --porcelain` — machine-readable status.
pub fn status_porcelain(project_path: &str) -> Result<String, AppError> {
    Ok(run_git(project_path, &["status", "--porcelain"])?
        .stdout)
}

/// `git commit -m <message>`.
pub fn commit(project_path: &str, message: &str) -> Result<(), AppError> {
    run_git(project_path, &["commit", "-m", message])?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Composite helpers
// ---------------------------------------------------------------------------

/// Parse `git diff --name-status` output into DiffFile entries.
fn parse_name_status(raw: &str) -> Vec<DiffFile> {
    raw.lines()
        .filter(|l| !l.is_empty())
        .filter_map(|line| {
            let mut parts = line.splitn(2, '\t');
            let status = parts.next()?.chars().next()?.to_string();
            let path = parts.next()?.to_string();
            Some(DiffFile { status, path })
        })
        .collect()
}

/// Run `git diff --name-status <range_args>` and parse the output into `DiffFile` entries.
pub fn diff_changed_files(
    project_path: &str,
    range_args: &[&str],
) -> Result<Vec<DiffFile>, AppError> {
    let mut args = vec!["diff", "--name-status"];
    args.extend_from_slice(range_args);
    let output = run_git(project_path, &args)?;
    Ok(parse_name_status(&output.stdout))
}

pub(crate) fn parse_stat(s: &str, keyword: &str) -> u64 {
    s.find(keyword)
        .and_then(|pos| {
            let before = &s[..pos];
            before
                .rfind(|c: char| !c.is_ascii_digit() && c != ' ')
                .map(|i| &before[i + 1..])
                .unwrap_or(before)
                .trim()
                .parse::<u64>()
                .ok()
        })
        .unwrap_or(0)
}
