use diesel::prelude::*;
use diesel::sqlite::SqliteConnection;

use crate::db::schema::reviews;
use crate::error::AppError;
use crate::models::review::Review;
use crate::repositories::{issue_repo, project_repo, review_repo};
use crate::utils::claude::run_claude;
use crate::utils::git::run_git;

pub struct IssueContext {
    pub project_path: String,
    pub issue_comment: String,
    pub reviews: Vec<Review>,
}

/// Gather all DB data needed to fix an issue. Call this while holding the DB lock.
pub fn gather_issue_context(
    conn: &mut SqliteConnection,
    project_id: &str,
    issue_id: &str,
) -> Result<IssueContext, AppError> {
    let issue = issue_repo::get(conn, issue_id)?;
    let project = project_repo::get(conn, project_id)?;

    let issue_id_owned = issue_id.to_string();
    let file_reviews = review_repo::list(conn, |q| {
        q.filter(reviews::issue_id.eq(issue_id_owned))
    })?;

    Ok(IssueContext {
        project_path: project.path,
        issue_comment: issue.comment,
        reviews: file_reviews,
    })
}

/// Build the prompt, run Claude, and commit. Does not need a DB connection.
pub fn execute_fix(ctx: &IssueContext) -> Result<String, AppError> {
    let mut affected_files_section = String::new();
    for review in &ctx.reviews {
        let Some(ref file_path) = review.relative_file_path else {
            continue;
        };

        let ref_path = format!("HEAD:{}", file_path);
        let content = run_git(&ctx.project_path, &["show", &ref_path])
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
        ctx.issue_comment, affected_files_section
    );

    let claude_output = run_claude(&ctx.project_path, &prompt)?;

    // Stage all changes and commit
    run_git(&ctx.project_path, &["add", "-A"])?;
    let commit_message = format!("fix: {}", ctx.issue_comment);
    run_git(&ctx.project_path, &["commit", "-m", &commit_message])?;

    Ok(claude_output)
}
