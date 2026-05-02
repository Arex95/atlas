-- Session-scoped memory: each session has its own key-value store.
CREATE TABLE IF NOT EXISTS session_memory (
    id         TEXT PRIMARY KEY,
    session_id TEXT NOT NULL REFERENCES ai_sessions(id) ON DELETE CASCADE,
    key        TEXT NOT NULL,
    value      TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (STRFTIME('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (STRFTIME('%Y-%m-%dT%H:%M:%fZ', 'now')),
    UNIQUE(session_id, key)
);

-- Session-scoped documents: each session has its own document space.
CREATE TABLE IF NOT EXISTS session_documents (
    id         TEXT PRIMARY KEY,
    session_id TEXT NOT NULL REFERENCES ai_sessions(id) ON DELETE CASCADE,
    title      TEXT NOT NULL,
    content    TEXT NOT NULL DEFAULT '',
    kind       TEXT NOT NULL DEFAULT 'document',
    created_at TEXT NOT NULL DEFAULT (STRFTIME('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (STRFTIME('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

-- Add optional session_id to tasks so a task can belong to a session or just a project.
ALTER TABLE tasks ADD COLUMN session_id TEXT REFERENCES ai_sessions(id) ON DELETE CASCADE;

-- Add optional session_id to reminders.
ALTER TABLE reminders ADD COLUMN session_id TEXT REFERENCES ai_sessions(id) ON DELETE CASCADE;
