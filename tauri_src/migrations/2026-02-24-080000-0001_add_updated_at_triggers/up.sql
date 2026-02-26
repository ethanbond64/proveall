CREATE TRIGGER projects_updated_at AFTER UPDATE ON projects
BEGIN
  UPDATE projects SET updated_at = CURRENT_TIMESTAMP WHERE id = NEW.id;
END;

CREATE TRIGGER events_updated_at AFTER UPDATE ON events
BEGIN
  UPDATE events SET updated_at = CURRENT_TIMESTAMP WHERE id = NEW.id;
END;

CREATE TRIGGER issues_updated_at AFTER UPDATE ON issues
BEGIN
  UPDATE issues SET updated_at = CURRENT_TIMESTAMP WHERE id = NEW.id;
END;

CREATE TRIGGER reviews_updated_at AFTER UPDATE ON reviews
BEGIN
  UPDATE reviews SET updated_at = CURRENT_TIMESTAMP WHERE id = NEW.id;
END;

CREATE TRIGGER composite_file_review_state_updated_at AFTER UPDATE ON composite_file_review_state
BEGIN
  UPDATE composite_file_review_state SET updated_at = CURRENT_TIMESTAMP WHERE id = NEW.id;
END;

CREATE TRIGGER branch_context_updated_at AFTER UPDATE ON branch_context
BEGIN
  UPDATE branch_context SET updated_at = CURRENT_TIMESTAMP WHERE id = NEW.id;
END;
