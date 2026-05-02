ALTER TABLE ai_sessions ADD COLUMN title TEXT;
ALTER TABLE ai_sessions ADD COLUMN author TEXT DEFAULT 'System';

CREATE TABLE IF NOT EXISTS session_messages (
    id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL REFERENCES ai_sessions(id) ON DELETE CASCADE,
    role TEXT NOT NULL, 
    content TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS session_messages_session_id_idx ON session_messages(session_id);
CREATE INDEX IF NOT EXISTS session_messages_created_at_idx ON session_messages(created_at);

ALTER TABLE projects ADD COLUMN author TEXT;
ALTER TABLE projects ADD COLUMN version TEXT DEFAULT '0.1.0';
