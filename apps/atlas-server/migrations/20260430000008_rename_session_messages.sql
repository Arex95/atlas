ALTER TABLE session_messages RENAME TO conversation_turns;

DROP INDEX IF EXISTS session_messages_session_id_idx;
DROP INDEX IF EXISTS session_messages_created_at_idx;

CREATE INDEX IF NOT EXISTS conversation_turns_session_id_idx ON conversation_turns(session_id);
CREATE INDEX IF NOT EXISTS conversation_turns_created_at_idx ON conversation_turns(created_at);
