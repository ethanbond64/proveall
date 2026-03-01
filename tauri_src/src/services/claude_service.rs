use diesel::prelude::*;
use diesel::sqlite::SqliteConnection;

use crate::db::schema::reviews;
use crate::error::AppError;
use crate::repositories::{issue_repo, project_repo, review_repo};
use crate::utils::claude::run_claude;
use crate::utils::git::run_git;

pub fn fix_issue(
    conn: &mut SqliteConnection,
    project_id: &str,
    issue_id: &str,
) -> Result<String, AppError> {
    let issue = issue_repo::get(conn, issue_id)?;
    let project = project_repo::get(conn, project_id)?;

    let issue_id_owned = issue_id.to_string();
    let file_reviews = review_repo::list(conn, |q| {
        q.filter(reviews::issue_id.eq(issue_id_owned))
    })?;

    let mut affected_files_section = String::new();
    for review in &file_reviews {
        let Some(ref file_path) = review.relative_file_path else {
            continue;
        };

        let ref_path = format!("HEAD:{}", file_path);
        let content = run_git(&project.path, &["show", &ref_path])
            .map(|o| o.stdout)
            .unwrap_or_default();

        affected_files_section.push_str(&format!("### File: {}\n", file_path));

        if let (Some(start), Some(end)) = (review.line_start, review.line_end) {
            let lines: Vec<&str> = content.lines().collect();
            let start_idx = (start as usize).saturating_sub(1);
            let end_idx = (end as usize).min(lines.len());
            let snippet = lines
                .get(start_idx..end_idx)
                .map(|s| s.join("\n"))
                .unwrap_or_default();
            affected_files_section.push_str(&format!(
                "Lines {}-{}:\n```\n{}\n```\n\n",
                start, end, snippet
            ));
        } else {
            affected_files_section.push_str(&format!("```\n{}\n```\n\n", content));
        }
    }

    let prompt = format!(
        "Fix the following code review issue.\n\n\
         ## Issue\n\
         {}\n\n\
         ## Affected files\n\
         {}\n\
         Fix the issue by editing the affected files. Only make changes necessary to address the issue.",
        issue.comment, affected_files_section
    );

    let claude_output = run_claude(&project.path, &prompt)?;

    // Stage all changes and commit
    run_git(&project.path, &["add", "-A"])?;
    let commit_message = format!("fix: {}", issue.comment);
    run_git(&project.path, &["commit", "-m", &commit_message])?;

    Ok(claude_output)
}
