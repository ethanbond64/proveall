use chrono::NaiveDateTime;
use diesel::prelude::*;
use serde::{Deserialize, Serialize};

use crate::db::schema::composite_file_review_state;
use crate::utils::hash_id;

#[derive(Queryable, Selectable, Serialize, Debug)]
#[diesel(table_name = composite_file_review_state)]
pub struct CompositeFileReviewState {
    pub id: String,
    pub project_id: String,
    pub relative_file_path: String,
    pub summary_metadata: String,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Insertable, Deserialize, Debug)]
#[diesel(table_name = composite_file_review_state)]
pub struct NewCompositeFileReviewState {
    pub id: String,
    pub project_id: String,
    pub relative_file_path: String,
    pub summary_metadata: String,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct ReviewSummaryMetadataEntry {
    pub start: i32,
    pub end: i32,
    pub state: String,
}

impl NewCompositeFileReviewState {
    pub fn new(project_id: String, relative_file_path: String, summary_metadata: String) -> Self {
        let now = chrono::Utc::now().naive_utc();
        let id = hash_id::composite_id(&project_id, &now);
        Self {
            id,
            project_id,
            relative_file_path,
            summary_metadata,
            created_at: now,
            updated_at: now,
        }
    }
}
