use chrono::NaiveDateTime;
use diesel::prelude::*;
use serde::{Deserialize, Serialize};

use crate::db::schema::events;
use crate::utils::hash_id;

#[derive(Queryable, Selectable, Serialize, Debug)]
#[diesel(table_name = events)]
pub struct Event {
    pub id: String,
    pub project_id: String,
    pub type_: String,
    pub hash: Option<String>,
    pub summary: String,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Insertable, Deserialize, Debug)]
#[diesel(table_name = events)]
pub struct NewEvent {
    pub id: String,
    pub project_id: String,
    pub type_: String,
    pub hash: Option<String>,
    pub summary: String,
}

impl NewEvent {
    pub fn new(project_id: String, type_: String, hash: Option<String>, summary: String) -> Self {
        let now = chrono::Utc::now().naive_utc();
        let id = hash_id::event_id(&project_id, &now);
        Self {
            id,
            project_id,
            type_,
            hash,
            summary,
        }
    }
}
