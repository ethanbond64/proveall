-- Drop tables in reverse order to respect foreign key constraints
DROP TABLE IF EXISTS event_issue_composite_xref;
DROP TABLE IF EXISTS branch_context;
DROP TABLE IF EXISTS reviews;
DROP TABLE IF EXISTS composite_file_review_state;
DROP TABLE IF EXISTS issues;
DROP TABLE IF EXISTS events;
DROP TABLE IF EXISTS projects;