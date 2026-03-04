// Insert a new record and return it.
macro_rules! impl_create {
    ($table_mod:ident, $entity:ty, $new_entity:ty) => {
        pub fn create(
            conn: &mut diesel::sqlite::SqliteConnection,
            new: $new_entity,
        ) -> Result<$entity, crate::error::AppError> {
            use diesel::prelude::*;
            diesel::insert_into(crate::db::schema::$table_mod::table)
                .values(&new)
                .returning(<$entity>::as_returning())
                .get_result(conn)
                .map_err(crate::error::AppError::from)
        }
    };
}

// Find one record by primary key. Errors if not found.
macro_rules! impl_get {
    ($table_mod:ident, $entity:ty) => {
        pub fn get(
            conn: &mut diesel::sqlite::SqliteConnection,
            id: &str,
        ) -> Result<$entity, crate::error::AppError> {
            use diesel::prelude::*;
            crate::db::schema::$table_mod::table
                .find(id)
                .select(<$entity>::as_select())
                .first(conn)
                .map_err(crate::error::AppError::from)
        }
    };
}

// Find one record matching a filter. Returns None if not found.
macro_rules! impl_find_by {
    ($table_mod:ident, $entity:ty) => {
        pub fn find_by(
            conn: &mut diesel::sqlite::SqliteConnection,
            filter: impl FnOnce(
                crate::db::schema::$table_mod::BoxedQuery<'_, diesel::sqlite::Sqlite>,
            )
                -> crate::db::schema::$table_mod::BoxedQuery<'_, diesel::sqlite::Sqlite>,
        ) -> Result<Option<$entity>, crate::error::AppError> {
            use diesel::prelude::*;
            let query = crate::db::schema::$table_mod::table.into_boxed();
            filter(query)
                .select(<$entity>::as_select())
                .first(conn)
                .optional()
                .map_err(crate::error::AppError::from)
        }
    };
}

// Return all records matching a filter.
macro_rules! impl_list {
    ($table_mod:ident, $entity:ty) => {
        pub fn list(
            conn: &mut diesel::sqlite::SqliteConnection,
            filter: impl FnOnce(
                crate::db::schema::$table_mod::BoxedQuery<'_, diesel::sqlite::Sqlite>,
            )
                -> crate::db::schema::$table_mod::BoxedQuery<'_, diesel::sqlite::Sqlite>,
        ) -> Result<Vec<$entity>, crate::error::AppError> {
            use diesel::prelude::*;
            let query = crate::db::schema::$table_mod::table.into_boxed();
            filter(query)
                .select(<$entity>::as_select())
                .load(conn)
                .map_err(crate::error::AppError::from)
        }
    };
}

// Update a record by primary key with a changeset and return it.
macro_rules! impl_update {
    ($table_mod:ident, $entity:ty) => {
        pub fn update<C>(
            conn: &mut diesel::sqlite::SqliteConnection,
            id: &str,
            changeset: C,
        ) -> Result<$entity, crate::error::AppError>
        where
            C: diesel::AsChangeset<Target = crate::db::schema::$table_mod::table>,
            C::Changeset: diesel::query_builder::QueryFragment<diesel::sqlite::Sqlite>,
        {
            use diesel::prelude::*;
            diesel::update(crate::db::schema::$table_mod::table.find(id))
                .set(changeset)
                .returning(<$entity>::as_returning())
                .get_result(conn)
                .map_err(crate::error::AppError::from)
        }
    };
}

// Join two tables, filter via closure, and return both entities as tuples.
macro_rules! impl_inner_join_list {
    ($source_mod:ident, $join_mod:ident, $source_entity:ty, $join_entity:ty) => {
        pub fn join_list(
            conn: &mut diesel::sqlite::SqliteConnection,
            filter: impl FnOnce(
                diesel::helper_types::IntoBoxed<
                    'static,
                    diesel::helper_types::InnerJoin<
                        crate::db::schema::$source_mod::table,
                        crate::db::schema::$join_mod::table,
                    >,
                    diesel::sqlite::Sqlite,
                >,
            ) -> diesel::helper_types::IntoBoxed<
                'static,
                diesel::helper_types::InnerJoin<
                    crate::db::schema::$source_mod::table,
                    crate::db::schema::$join_mod::table,
                >,
                diesel::sqlite::Sqlite,
            >,
        ) -> Result<Vec<($source_entity, $join_entity)>, crate::error::AppError> {
            use diesel::prelude::*;
            // Box the joined query so the filter closure can apply
            // .filter(), .distinct(), .order(), etc.
            let query = crate::db::schema::$source_mod::table
                .inner_join(crate::db::schema::$join_mod::table)
                .into_boxed();
            // Apply caller's filter, then select both entities as a tuple
            filter(query)
                .select((<$source_entity>::as_select(), <$join_entity>::as_select()))
                .load(conn)
                .map_err(crate::error::AppError::from)
        }
    };
}

pub mod project_repo {
    use crate::models::project::{NewProject, Project};
    impl_create!(projects, Project, NewProject);
    impl_get!(projects, Project);
    impl_find_by!(projects, Project);
    impl_list!(projects, Project);
    impl_update!(projects, Project);
}

pub mod branch_context_repo {
    use crate::models::branch_context::{BranchContext, NewBranchContext};
    impl_create!(branch_context, BranchContext, NewBranchContext);
    impl_get!(branch_context, BranchContext);
    impl_update!(branch_context, BranchContext);
}

pub mod event_repo {
    use crate::models::event::{Event, NewEvent};
    impl_create!(events, Event, NewEvent);
    impl_get!(events, Event);
    impl_list!(events, Event);
}

pub mod issue_repo {
    use crate::models::event_issue_composite_xref::EventIssueCompositeXref;
    use crate::models::issue::{Issue, NewIssue};
    impl_create!(issues, Issue, NewIssue);
    impl_get!(issues, Issue);
    #[cfg(test)]
    impl_list!(issues, Issue);
    impl_update!(issues, Issue);
    impl_inner_join_list!(
        event_issue_composite_xref,
        issues,
        EventIssueCompositeXref,
        Issue
    );
}

pub mod composite_file_review_state_repo {
    use crate::models::composite_file_review_state::{
        CompositeFileReviewState, NewCompositeFileReviewState,
    };
    impl_create!(
        composite_file_review_state,
        CompositeFileReviewState,
        NewCompositeFileReviewState
    );
}

pub mod review_repo {
    use crate::models::review::{NewReview, Review};
    impl_create!(reviews, Review, NewReview);
}

pub mod event_issue_composite_xref_repo {
    use crate::models::composite_file_review_state::CompositeFileReviewState;
    use crate::models::event_issue_composite_xref::EventIssueCompositeXref;
    impl_create!(
        event_issue_composite_xref,
        EventIssueCompositeXref,
        EventIssueCompositeXref
    );
    impl_inner_join_list!(
        event_issue_composite_xref,
        composite_file_review_state,
        EventIssueCompositeXref,
        CompositeFileReviewState
    );

    pub fn join_list_by_event(
        conn: &mut diesel::sqlite::SqliteConnection,
        event_id: &str,
        branch_context_id: &str,
    ) -> Result<Vec<(EventIssueCompositeXref, CompositeFileReviewState)>, crate::error::AppError>
    {
        use diesel::prelude::*;
        let eid = event_id.to_string();
        let bcid = branch_context_id.to_string();
        join_list(conn, |q| {
            q.filter(
                crate::db::schema::event_issue_composite_xref::event_id
                    .eq(eid)
                    .and(crate::db::schema::event_issue_composite_xref::branch_context_id.eq(bcid)),
            )
        })
    }

    pub fn join_list_by_event_and_issue(
        conn: &mut diesel::sqlite::SqliteConnection,
        event_id: &str,
        issue_id: &str,
        branch_context_id: &str,
    ) -> Result<Vec<(EventIssueCompositeXref, CompositeFileReviewState)>, crate::error::AppError>
    {
        use diesel::prelude::*;
        let eid = event_id.to_string();
        let iid = issue_id.to_string();
        let bcid = branch_context_id.to_string();
        join_list(conn, |q| {
            q.filter(
                crate::db::schema::event_issue_composite_xref::event_id
                    .eq(eid)
                    .and(crate::db::schema::event_issue_composite_xref::issue_id.eq(iid))
                    .and(crate::db::schema::event_issue_composite_xref::branch_context_id.eq(bcid)),
            )
        })
    }
}
