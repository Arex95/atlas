-- Fix FK references broken by the failed sessions_cascade migration.
-- When ai_sessions was temporarily renamed to ai_sessions_old, SQLite
-- automatically updated the FK text in child tables. After ai_sessions was
-- restored, those tables still point to the non-existent ai_sessions_old.
-- Affected tables: conversation_turns, messages, session_scrollback.

PRAGMA foreign_keys = OFF;

-- conversation_turns
ALTER TABLE conversation_turns RENAME TO conversation_turns_old;
CREATE TABLE conversation_turns (
    id         TEXT PRIMARY KEY,
    session_id TEXT NOT NULL REFERENCES ai_sessions(id) ON DELETE CASCADE,
    role       TEXT NOT NULL,
    content    TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
INSERT INTO conversation_turns SELECT * FROM conversation_turns_old;
DROP TABLE conversation_turns_old;
CREATE INDEX IF NOT EXISTS conversation_turns_session_id_idx ON conversation_turns(session_id);
CREATE INDEX IF NOT EXISTS conversation_turns_created_at_idx ON conversation_turns(created_at);

-- messages
ALTER TABLE messages RENAME TO messages_old;
CREATE TABLE messages (
    id         TEXT PRIMARY KEY,
    session_id TEXT NOT NULL REFERENCES ai_sessions(id) ON DELETE CASCADE,
    from_id    TEXT NOT NULL,
    content    TEXT NOT NULL,
    timestamp  DATETIME DEFAULT CURRENT_TIMESTAMP,
    is_read    BOOLEAN DEFAULT 0
);
INSERT INTO messages SELECT * FROM messages_old;
DROP TABLE messages_old;

-- session_scrollback
ALTER TABLE session_scrollback RENAME TO session_scrollback_old;
CREATE TABLE session_scrollback (
    session_id TEXT PRIMARY KEY REFERENCES ai_sessions(id) ON DELETE CASCADE,
    content    TEXT NOT NULL DEFAULT '',
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
INSERT INTO session_scrollback SELECT * FROM session_scrollback_old;
DROP TABLE session_scrollback_old;

PRAGMA foreign_keys = ON;
