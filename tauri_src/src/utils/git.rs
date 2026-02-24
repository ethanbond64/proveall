use std::process::Command;

use serde::Serialize;

use crate::error::AppError;

pub(crate) struct GitOutput {
    pub stdout: String,
    pub _stderr: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiffFile {
    pub(crate) status: String,
    pub(crate) path: String,
}

pub(crate) fn run_git(project_path: &str, args: &[&str]) -> Result<GitOutput, AppError> {
    let output = Command::new("git")
        .args(args)
        .current_dir(project_path)
        .output()?;

    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();

    if output.status.success() {
        Ok(GitOutput { stdout, _stderr: stderr })
    } else {
        Err(AppError::Git(stderr))
    }
}

/// Parse `git diff --name-status` output into DiffFile entries.
pub(crate) fn parse_name_status(raw: &str) -> Vec<DiffFile> {
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
pub fn diff_changed_files(project_path: &str, range_args: &[&str]) -> Result<Vec<DiffFile>, AppError> {
    let mut args = vec!["diff", "--name-status"];
    args.extend_from_slice(range_args);
    let output = run_git(project_path, &args)?;
    Ok(parse_name_status(&output.stdout))
}

pub(crate) fn parse_stat(s: &str, keyword: &str) -> u64 {
    s.find(keyword).and_then(|pos| {
        let before = &s[..pos];
        before.rfind(|c: char| !c.is_ascii_digit() && c != ' ')
            .map(|i| &before[i + 1..])
            .unwrap_or(before)
            .trim()
            .parse::<u64>()
            .ok()
    }).unwrap_or(0)
}
