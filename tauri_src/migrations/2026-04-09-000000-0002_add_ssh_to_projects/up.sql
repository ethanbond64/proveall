ALTER TABLE projects ADD COLUMN connection_type TEXT NOT NULL DEFAULT 'local';
ALTER TABLE projects ADD COLUMN ssh_config TEXT;
