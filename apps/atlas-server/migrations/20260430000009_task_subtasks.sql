ALTER TABLE tasks ADD COLUMN parent_id TEXT REFERENCES tasks(id) ON DELETE CASCADE;
CREATE INDEX IF NOT EXISTS tasks_parent_id_idx ON tasks(parent_id);
