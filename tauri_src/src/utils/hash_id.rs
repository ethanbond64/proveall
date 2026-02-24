use chrono::NaiveDateTime;
use sha2::{Digest, Sha256};

/// Truncate a SHA256 hash to 16 hex chars (64-bit entropy).
fn hash_to_id(input: &str) -> String {
    let digest = Sha256::digest(input.as_bytes());
    hex::encode(digest)[..16].to_string()
}

/// Deterministic: same path always produces the same project ID.
pub fn project_id(path: &str) -> String {
    hash_to_id(&format!("project:{}", path))
}

/// Per-creation: includes created_at to ensure uniqueness.
pub fn event_id(project_id: &str, created_at: &NaiveDateTime) -> String {
    hash_to_id(&format!("event:{}:{}", project_id, created_at))
}

/// Per-creation: includes created_at to ensure uniqueness.
pub fn issue_id(event_id: &str, created_at: &NaiveDateTime) -> String {
    hash_to_id(&format!("issue:{}:{}", event_id, created_at))
}

/// Per-creation: includes created_at to ensure uniqueness.
pub fn review_id(issue_id: &str, created_at: &NaiveDateTime) -> String {
    hash_to_id(&format!("review:{}:{}", issue_id, created_at))
}

/// Per-creation: includes created_at to ensure uniqueness.
pub fn composite_id(project_id: &str, created_at: &NaiveDateTime) -> String {
    hash_to_id(&format!("composite:{}:{}", project_id, created_at))
}

/// Deterministic: same project + branch + base_branch always produces the same ID.
pub fn branch_context_id(project_id: &str, branch: &str, base_branch: &str) -> String {
    hash_to_id(&format!("branch_context:{}:{}:{}", project_id, branch, base_branch))
}
