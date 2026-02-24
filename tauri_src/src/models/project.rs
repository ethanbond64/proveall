use chrono::NaiveDateTime;
use diesel::prelude::*;
use serde::{Deserialize, Serialize};

use crate::db::schema::projects;
use crate::utils::hash_id;

#[derive(Queryable, Selectable, Serialize, Debug)]
#[diesel(table_name = projects)]
pub struct Project {
    pub id: String,
    pub path: String,
    pub name: Option<String>,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
    pub last_opened: Option<NaiveDateTime>,
}

#[derive(Insertable, Deserialize, Debug)]
#[diesel(table_name = projects)]
pub struct NewProject {
    pub id: String,
    pub path: String,
    pub name: Option<String>,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
    pub last_opened: Option<NaiveDateTime>,
}

impl NewProject {
    pub fn new(path: String, name: Option<String>) -> Self {
        let now = chrono::Utc::now().naive_utc();
        let id = hash_id::project_id(&path);
        Self {
            id,
            path,
            name,
            created_at: now,
            updated_at: now,
            last_opened: Some(now),
        }
    }
}

#[derive(AsChangeset, Debug)]
#[diesel(table_name = projects)]
pub struct ProjectLastOpenedUpdate {
    pub updated_at: NaiveDateTime,
    pub last_opened: Option<NaiveDateTime>,
}
