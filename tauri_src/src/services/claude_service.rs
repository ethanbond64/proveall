use diesel::sqlite::SqliteConnection;

use crate::error::AppError;
use crate::models::composite_file_review_state::{
    CompositeFileReviewState, ReviewSummaryMetadataEntry,
};
use crate::repositories::{
    branch_context_repo, event_issue_composite_xref_repo, issue_repo, project_repo,
};
use crate::utils::claude::run_claude;
use crate::utils::git::run_git;

pub struct IssueContext {
    pub project_path: String,
    pub issue_comment: String,
    pub composites: Vec<CompositeFileReviewState>,
}

/// Gather all DB data needed to fix an issue. Call this while holding the DB lock.
pub fn gather_issue_context(
    conn: &mut SqliteConnection,
    project_id: &str,
    issue_id: &str,
    branch_context_id: &str,
) -> Result<IssueContext, AppError> {
    let issue = issue_repo::get(conn, issue_id)?;
    let project = project_repo::get(conn, project_id)?;
    let branch_context = branch_context_repo::get(conn, branch_context_id)?;

    let composites = if let Some(ref event_id) = branch_context.head_event_id {
        event_issue_composite_xref_repo::join_list_by_event_and_issue(
            conn,
            event_id,
            issue_id,
            branch_context_id,
        )?
        .into_iter()
        .map(|(_, composite)| composite)
        .collect()
    } else {
        vec![]
    };

    Ok(IssueContext {
        project_path: project.path,
        issue_comment: issue.comment,
        composites,
    })
}

/// Build the prompt, run Claude, and commit. Does not need a DB connection.
pub fn execute_fix(ctx: &IssueContext) -> Result<String, AppError> {
    let mut affected_files_section = String::new();
    for composite in &ctx.composites {
        let file_path = &composite.relative_file_path;
        let full_path = std::path::Path::new(&ctx.project_path).join(file_path);
        let content = std::fs::read_to_string(&full_path).unwrap_or_default();

        affected_files_section.push_str(&format!("### File: {}\n", file_path));

        let entries: Vec<ReviewSummaryMetadataEntry> =
            serde_json::from_str(&composite.summary_metadata).unwrap_or_default();

        if !entries.is_empty() {
            affected_files_section.push_str("Review state:\n");
            for entry in &entries {
                affected_files_section.push_str(&format!(
                    "- Lines {}-{}: {}\n",
                    entry.start, entry.end, entry.state
                ));
            }
            affected_files_section.push('\n');
        }

        affected_files_section.push_str(&format!("```\n{}\n```\n\n", content));
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
