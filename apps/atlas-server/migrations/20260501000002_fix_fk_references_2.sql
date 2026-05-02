-- Fix remaining FK references broken by the failed sessions_cascade migration.
-- notifications and prompts still reference ai_sessions_old.

PRAGMA foreign_keys = OFF;

-- notifications
ALTER TABLE notifications RENAME TO notifications_old;
CREATE TABLE notifications (
    id         TEXT PRIMARY KEY,
    project_id TEXT REFERENCES projects(id) ON DELETE CASCADE,
    session_id TEXT REFERENCES ai_sessions(id) ON DELETE SET NULL,
    message    TEXT NOT NULL,
    type       TEXT NOT NULL DEFAULT 'info',
    status     TEXT NOT NULL DEFAULT 'unread',
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    title      TEXT
);
INSERT INTO notifications SELECT * FROM notifications_old;
DROP TABLE notifications_old;

-- prompts
ALTER TABLE prompts RENAME TO prompts_old;
CREATE TABLE prompts (
    id         TEXT PRIMARY KEY,
    project_id TEXT REFERENCES projects(id) ON DELETE CASCADE,
    session_id TEXT REFERENCES ai_sessions(id) ON DELETE CASCADE,
    title      TEXT NOT NULL,
    content    TEXT NOT NULL,
    category   TEXT NOT NULL DEFAULT 'general',
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
INSERT INTO prompts SELECT * FROM prompts_old;
DROP TABLE prompts_old;

PRAGMA foreign_keys = ON;
