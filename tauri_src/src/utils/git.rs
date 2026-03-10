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
    Ok(
        run_git(project_path, &["rev-parse", "--abbrev-ref", "HEAD"])?
            .stdout
            .trim()
            .to_string(),
    )
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
    Ok(run_git(project_path, &["diff", "--shortstat", range])?.stdout)
}

pub struct GitCommit {
    pub hash: String,
    pub subject: String,
    pub author: String,
    pub date: String,
    pub parents: Vec<String>, // empty for root, 1 for normal, 2+ for merge
}

/// Structured git log. Extra args (e.g. "--no-merges", range, "-1 <hash>")
/// are appended after the format flag.
pub fn log(project_path: &str, args: &[&str]) -> Result<Vec<GitCommit>, AppError> {
    let mut cmd_args = vec!["log", "--format=%H%n%P%n%s%n%an%n%aI"];
    cmd_args.extend_from_slice(args);
    let output = run_git(project_path, &cmd_args)?;
    let lines: Vec<&str> = output.stdout.lines().collect();
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
    Ok(run_git(project_path, &["diff", prev, curr, "--", file_path])?.stdout)
}

/// `git merge-base --is-ancestor <ancestor> <descendant>` — returns true if ancestor
/// is an ancestor of descendant.
pub fn is_ancestor(project_path: &str, ancestor: &str, descendant: &str) -> bool {
    run_git(
        project_path,
        &["merge-base", "--is-ancestor", ancestor, descendant],
    )
    .is_ok()
}

/// Classify whether a merge commit is a base-branch merge.
/// A merge commit is a "base-branch merge" if any of its parents is an ancestor
/// of the base branch tip — meaning the user merged base into their feature branch.
pub fn is_base_branch_merge(project_path: &str, parents: &[String], base_branch: &str) -> bool {
    let base_hash = match rev_parse(project_path, base_branch) {
        Ok(h) => h,
        Err(_) => return false,
    };
    parents
        .iter()
        .any(|p| is_ancestor(project_path, p, &base_hash))
}

/// `git diff-tree --cc -p <hash>` — combined diff for a merge commit.
/// The first line of output is always the commit hash, so we skip it.
/// After skipping, non-empty output indicates conflict resolution changes.
pub fn diff_tree_cc(project_path: &str, hash: &str) -> Result<String, AppError> {
    let output = run_git(project_path, &["diff-tree", "--cc", "-p", hash])?;
    Ok(output.stdout.lines().skip(1).collect::<Vec<_>>().join("\n"))
}

/// `git diff-tree --cc --name-only <hash>` — list files with conflict changes only.
/// The first line of output is the commit hash, so we skip it.
pub fn diff_tree_cc_name_only(project_path: &str, hash: &str) -> Result<String, AppError> {
    let output = run_git(project_path, &["diff-tree", "--cc", "--name-only", hash])?;
    // First line is the commit hash — skip it
    Ok(output.stdout.lines().skip(1).collect::<Vec<_>>().join("\n"))
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
