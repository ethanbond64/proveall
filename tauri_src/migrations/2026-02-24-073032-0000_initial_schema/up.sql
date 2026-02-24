-- Create projects table
CREATE TABLE projects (
    id TEXT PRIMARY KEY NOT NULL,
    path TEXT NOT NULL,
    name TEXT,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    last_opened TIMESTAMP
);

-- Create events table
CREATE TABLE events (
    id TEXT PRIMARY KEY NOT NULL,
    project_id TEXT NOT NULL,
    type TEXT NOT NULL,
    hash TEXT,
    summary TEXT NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (project_id) REFERENCES projects(id)
);

-- Create issues table
CREATE TABLE issues (
    id TEXT PRIMARY KEY NOT NULL,
    project_id TEXT NOT NULL,
    created_event_id TEXT NOT NULL,
    resolved_event_id TEXT,
    comment TEXT NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (project_id) REFERENCES projects(id),
    FOREIGN KEY (created_event_id) REFERENCES events(id),
    FOREIGN KEY (resolved_event_id) REFERENCES events(id)
);

-- Create reviews table
CREATE TABLE reviews (
    id TEXT PRIMARY KEY NOT NULL,
    issue_id TEXT NOT NULL,
    type TEXT NOT NULL,
    relative_file_path TEXT,
    line_start INTEGER,
    line_end INTEGER,
    dimension TEXT NOT NULL,
    dimension_value TEXT NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (issue_id) REFERENCES issues(id)
);

-- Create composite_file_review_state table
CREATE TABLE composite_file_review_state (
    id TEXT PRIMARY KEY NOT NULL,
    project_id TEXT NOT NULL,
    relative_file_path TEXT NOT NULL,
    summary_metadata TEXT NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (project_id) REFERENCES projects(id)
);

-- Create branch_context table
CREATE TABLE branch_context (
    id TEXT PRIMARY KEY NOT NULL,
    project_id TEXT NOT NULL,
    branch TEXT NOT NULL,
    base_branch TEXT NOT NULL,
    head_event_id TEXT,
    settings TEXT NOT NULL DEFAULT '{}',
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (project_id) REFERENCES projects(id),
    FOREIGN KEY (head_event_id) REFERENCES events(id)
);

-- Create event_issue_composite_xref table
CREATE TABLE event_issue_composite_xref (
    event_id TEXT NOT NULL,
    issue_id TEXT NOT NULL,
    composite_file_id TEXT NOT NULL,
    branch_context_id TEXT NOT NULL,
    PRIMARY KEY (event_id, issue_id, composite_file_id),
    FOREIGN KEY (event_id) REFERENCES events(id),
    FOREIGN KEY (issue_id) REFERENCES issues(id),
    FOREIGN KEY (composite_file_id) REFERENCES composite_file_review_state(id),
    FOREIGN KEY (branch_context_id) REFERENCES branch_context(id)
);