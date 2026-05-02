-- Make project_id nullable on tasks so session-scoped tasks (no project_id) are valid.
-- SQLite does not support ALTER COLUMN, so we recreate the table.

CREATE TABLE tasks_new (
    id          TEXT PRIMARY KEY,
    project_id  TEXT REFERENCES projects(id) ON DELETE CASCADE,
    session_id  TEXT REFERENCES ai_sessions(id) ON DELETE CASCADE,
    title       TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    status      TEXT NOT NULL DEFAULT 'todo',
    priority    TEXT NOT NULL DEFAULT 'medium',
    due_date    TEXT,
    assigned_to TEXT,
    tags        TEXT NOT NULL DEFAULT '[]',
    parent_id   TEXT REFERENCES tasks_new(id) ON DELETE CASCADE,
    created_at  TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at  TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

INSERT INTO tasks_new (id, project_id, session_id, title, description, status, priority, due_date, assigned_to, tags, created_at, updated_at)
SELECT id, project_id, session_id, title, description, status, priority, due_date, assigned_to, tags, created_at, updated_at
FROM tasks;

DROP TABLE tasks;
ALTER TABLE tasks_new RENAME TO tasks;

CREATE INDEX IF NOT EXISTS tasks_project_idx   ON tasks(project_id);
CREATE INDEX IF NOT EXISTS tasks_status_idx    ON tasks(status);
CREATE INDEX IF NOT EXISTS tasks_due_date_idx  ON tasks(due_date);
CREATE INDEX IF NOT EXISTS tasks_session_idx   ON tasks(session_id);
