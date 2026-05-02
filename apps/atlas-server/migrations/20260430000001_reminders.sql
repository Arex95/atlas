CREATE TABLE IF NOT EXISTS reminders (
    id TEXT PRIMARY KEY,
    project_id TEXT REFERENCES projects(id) ON DELETE CASCADE,
    title TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    due_at TEXT NOT NULL,
    kind TEXT NOT NULL DEFAULT 'reminder',
    status TEXT NOT NULL DEFAULT 'pending',
    last_notified_at TEXT,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS reminders_project_id_idx ON reminders(project_id);
CREATE INDEX IF NOT EXISTS reminders_due_at_idx ON reminders(due_at);
CREATE INDEX IF NOT EXISTS reminders_status_idx ON reminders(status);
