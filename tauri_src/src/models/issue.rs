use chrono::NaiveDateTime;
use diesel::prelude::*;
use serde::{Deserialize, Serialize};

use crate::db::schema::issues;
use crate::utils::hash_id;

#[derive(Queryable, Selectable, Serialize, Debug)]
#[diesel(table_name = issues)]
pub struct Issue {
    pub id: String,
    pub project_id: String,
    pub created_event_id: String,
    pub resolved_event_id: Option<String>,
    pub comment: String,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Insertable, Deserialize, Debug)]
#[diesel(table_name = issues)]
pub struct NewIssue {
    pub id: String,
    pub project_id: String,
    pub created_event_id: String,
    pub comment: String,
}

impl NewIssue {
    pub fn new(project_id: String, created_event_id: String, comment: String) -> Self {
        let now = chrono::Utc::now().naive_utc();
        let id = hash_id::issue_id(&created_event_id, &now);
        Self {
            id,
            project_id,
            created_event_id,
            comment,
        }
    }
}
