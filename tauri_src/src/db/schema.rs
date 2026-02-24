// @generated automatically by Diesel CLI.

diesel::table! {
    use diesel::sql_types::*;

    branch_context (id) {
        id -> Text,
        project_id -> Text,
        branch -> Text,
        base_branch -> Text,
        head_event_id -> Nullable<Text>,
        settings -> Text,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

diesel::table! {
    use diesel::sql_types::*;

    composite_file_review_state (id) {
        id -> Text,
        project_id -> Text,
        relative_file_path -> Text,
        summary_metadata -> Text,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

diesel::table! {
    use diesel::sql_types::*;

    event_issue_composite_xref (event_id, issue_id, composite_file_id) {
        event_id -> Text,
        issue_id -> Text,
        composite_file_id -> Text,
        branch_context_id -> Text,
    }
}

diesel::table! {
    use diesel::sql_types::*;

    events (id) {
        id -> Text,
        project_id -> Text,
        #[sql_name = "type"]
        type_ -> Text,
        hash -> Nullable<Text>,
        summary -> Text,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

diesel::table! {
    use diesel::sql_types::*;

    issues (id) {
        id -> Text,
        project_id -> Text,
        created_event_id -> Text,
        resolved_event_id -> Nullable<Text>,
        comment -> Text,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

diesel::table! {
    use diesel::sql_types::*;

    projects (id) {
        id -> Text,
        path -> Text,
        name -> Nullable<Text>,
        created_at -> Timestamp,
        updated_at -> Timestamp,
        last_opened -> Nullable<Timestamp>,
    }
}

diesel::table! {
    use diesel::sql_types::*;

    reviews (id) {
        id -> Text,
        issue_id -> Text,
        #[sql_name = "type"]
        type_ -> Text,
        relative_file_path -> Nullable<Text>,
        line_start -> Nullable<Integer>,
        line_end -> Nullable<Integer>,
        dimension -> Text,
        dimension_value -> Text,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

diesel::joinable!(branch_context -> events (head_event_id));
diesel::joinable!(branch_context -> projects (project_id));
diesel::joinable!(composite_file_review_state -> projects (project_id));
diesel::joinable!(event_issue_composite_xref -> branch_context (branch_context_id));
diesel::joinable!(event_issue_composite_xref -> composite_file_review_state (composite_file_id));
diesel::joinable!(event_issue_composite_xref -> events (event_id));
diesel::joinable!(event_issue_composite_xref -> issues (issue_id));
diesel::joinable!(events -> projects (project_id));
diesel::joinable!(issues -> projects (project_id));
diesel::joinable!(reviews -> issues (issue_id));

diesel::allow_tables_to_appear_in_same_query!(
    branch_context,
    composite_file_review_state,
    event_issue_composite_xref,
    events,
    issues,
    projects,
    reviews,
);
