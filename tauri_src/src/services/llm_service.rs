use diesel::sqlite::SqliteConnection;

use crate::error::AppError;
use crate::models::composite_file_review_state::{
    CompositeFileReviewState, ReviewSummaryMetadataEntry,
};
use crate::repositories::{branch_context_repo, event_issue_composite_xref_repo, issue_repo};

pub struct IssueContext {
    pub issue_comment: String,
    pub composites: Vec<CompositeFileReviewState>,
}

/// Gather all DB data needed to build an issue prompt. Call this while holding the DB lock.
pub fn gather_issue_context(
    conn: &mut SqliteConnection,
    issue_id: &str,
    branch_context_id: &str,
) -> Result<IssueContext, AppError> {
    let issue = issue_repo::get(conn, issue_id)?;
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
        issue_comment: issue.comment,
        composites,
    })
}

/// Build the prompt string from issue context and a user-configurable template.
/// The template should contain `{issue}` which gets replaced with the issue details.
pub fn build_prompt(ctx: &IssueContext, template: &str) -> String {
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

    let issue_block = format!(
        "## Issue\n{}\n\n## Affected files\n{}",
        ctx.issue_comment, affected_files
    );

    template.replace("{issue}", &issue_block)
}
