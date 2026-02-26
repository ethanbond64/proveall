use chrono::NaiveDateTime;
use diesel::prelude::*;
use serde::{Deserialize, Serialize};

use crate::db::schema::reviews;
use crate::utils::hash_id;

#[derive(Queryable, Selectable, Serialize, Debug)]
#[diesel(table_name = reviews)]
pub struct Review {
    pub id: String,
    pub issue_id: String,
    pub type_: String,
    pub relative_file_path: Option<String>,
    pub line_start: Option<i32>,
    pub line_end: Option<i32>,
    pub dimension: String,
    pub dimension_value: String,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Insertable, Deserialize, Debug)]
#[diesel(table_name = reviews)]
pub struct NewReview {
    pub id: String,
    pub issue_id: String,
    pub type_: String,
    pub relative_file_path: Option<String>,
    pub line_start: Option<i32>,
    pub line_end: Option<i32>,
    pub dimension: String,
    pub dimension_value: String,
}

impl NewReview {
    pub fn new(
        issue_id: String,
        type_: String,
        relative_file_path: Option<String>,
        line_start: Option<i32>,
        line_end: Option<i32>,
        dimension: String,
        dimension_value: String,
    ) -> Self {
        let now = chrono::Utc::now().naive_utc();
        let id = hash_id::review_id(&issue_id, &now);
        Self {
            id,
            issue_id,
            type_,
            relative_file_path,
            line_start,
            line_end,
            dimension,
            dimension_value,
        }
    }
}
