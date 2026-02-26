use chrono::NaiveDateTime;
use diesel::prelude::*;
use serde::{Deserialize, Serialize};

use crate::db::schema::branch_context;
use crate::utils::hash_id;

#[derive(Queryable, Selectable, Serialize, Debug)]
#[diesel(table_name = branch_context)]
pub struct BranchContext {
    pub id: String,
    pub project_id: String,
    pub branch: String,
    pub base_branch: String,
    pub head_event_id: Option<String>,
    pub settings: String,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Insertable, Deserialize, Debug)]
#[diesel(table_name = branch_context)]
pub struct NewBranchContext {
    pub id: String,
    pub project_id: String,
    pub branch: String,
    pub base_branch: String,
    pub head_event_id: Option<String>,
    pub settings: String,
}

impl NewBranchContext {
    pub fn new(project_id: String, branch: String, base_branch: String, settings: String) -> Self {
        let id = hash_id::branch_context_id(&project_id, &branch, &base_branch);
        Self {
            id,
            project_id,
            branch,
            base_branch,
            head_event_id: None,
            settings,
        }
    }
}
