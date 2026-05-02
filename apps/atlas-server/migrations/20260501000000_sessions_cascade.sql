-- Recreate ai_sessions with ON DELETE CASCADE on project_id.
-- SQLite cannot ALTER foreign key constraints, so we rename + recreate.
-- All 22 columns included (init + metadata + customization + enhancements).

PRAGMA foreign_keys = OFF;

ALTER TABLE ai_sessions RENAME TO ai_sessions_old;

CREATE TABLE ai_sessions (
    id                  TEXT PRIMARY KEY,
    project_id          TEXT REFERENCES projects(id) ON DELETE CASCADE,
    provider            TEXT NOT NULL,
    model               TEXT NOT NULL,
    status              TEXT NOT NULL DEFAULT 'starting',
    pid                 INTEGER,
    pty_fd              INTEGER,
    working_directory   TEXT NOT NULL,
    prompt              TEXT,
    mode                TEXT NOT NULL DEFAULT 'interactive',
    linked_task_id      TEXT,
    started_at          TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    stopped_at          TEXT,
    last_activity_at    TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    title               TEXT,
    author              TEXT DEFAULT 'System',
    is_saved            INTEGER DEFAULT 0,
    custom_name         TEXT,
    custom_description  TEXT,
    color               TEXT,
    icon                TEXT,
    tags                TEXT NOT NULL DEFAULT '[]'
);

INSERT INTO ai_sessions SELECT * FROM ai_sessions_old;
DROP TABLE ai_sessions_old;

CREATE INDEX IF NOT EXISTS ai_sessions_project_id_idx  ON ai_sessions(project_id);
CREATE INDEX IF NOT EXISTS ai_sessions_status_idx      ON ai_sessions(status);
CREATE INDEX IF NOT EXISTS ai_sessions_provider_idx    ON ai_sessions(provider);
CREATE INDEX IF NOT EXISTS ai_sessions_linked_task_idx ON ai_sessions(linked_task_id);

PRAGMA foreign_keys = ON;
