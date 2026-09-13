-- Inter-session coordination protocol. `to_session`
-- NULL means a broadcast to the project channel (project state,
-- shared); a value means a direct message (Type 2, personal) —
-- that single column already carries the classification, no
-- separate `kind` column needed.
--
-- `id` is a ULID (repo convention: sortable, not UUID), so it
-- doubles as the ordering/cursor key for read_inbox — no separate
-- sequence or timestamp comparison needed.
CREATE TABLE messages (
    id             TEXT NOT NULL PRIMARY KEY,
    project        TEXT NOT NULL,
    from_session   TEXT NOT NULL,
    to_session     TEXT,
    message_type   TEXT NOT NULL,
    payload_json   TEXT NOT NULL,
    correlation_id TEXT,
    reply_to       TEXT,
    created_at     TEXT NOT NULL
);

CREATE INDEX idx_messages_project_id ON messages (project, id);
