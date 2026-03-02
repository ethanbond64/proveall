use diesel::sqlite::SqliteConnection;

use crate::error::AppError;
use crate::models::composite_file_review_state::{
    CompositeFileReviewState, ReviewSummaryMetadataEntry,
};
use crate::repositories::{
    branch_context_repo, event_issue_composite_xref_repo, issue_repo, project_repo,
};
use crate::utils::git::run_git;
use crate::utils::llm::LlmProvider;

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

/// Build the prompt, run the LLM, and commit. Does not need a DB connection.
pub fn execute_fix(ctx: &IssueContext, provider: &dyn LlmProvider) -> Result<String, AppError> {
    let mut affected_files = String::new();
    for composite in &ctx.composites {
        affected_files.push_str(&format!("### {}\n", composite.relative_file_path));

        let entries: Vec<ReviewSummaryMetadataEntry> =
            serde_json::from_str(&composite.summary_metadata).unwrap_or_default();
        for entry in &entries {
            affected_files.push_str(&format!(
                "- Lines {}-{}: {}\n",
                entry.start, entry.end, entry.state
            ));
        }
        affected_files.push('\n');
    }

    let prompt = format!(
        "Fix the following code review issue.\n\n\
         ## Issue\n\
         {}\n\n\
         ## Affected files\n\
         {}\
         Fix the issue by editing the affected files. Only make changes necessary to address the issue.",
        ctx.issue_comment, affected_files
    );

    let llm_output = provider.run(&ctx.project_path, &prompt)?;

    run_git(&ctx.project_path, &["add", "-A"])?;
    run_git(
        &ctx.project_path,
        &["commit", "-m", &format!("fix: {}", ctx.issue_comment)],
    )?;

    Ok(llm_output)
}
