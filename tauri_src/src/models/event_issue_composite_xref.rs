use diesel::prelude::*;
use serde::{Deserialize, Serialize};

use crate::db::schema::event_issue_composite_xref;

#[derive(Queryable, Selectable, Insertable, Serialize, Deserialize, Debug)]
#[diesel(table_name = event_issue_composite_xref)]
pub struct EventIssueCompositeXref {
    pub event_id: String,
    pub issue_id: String,
    pub composite_file_id: String,
    pub branch_context_id: String,
}
