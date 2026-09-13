-- Credentials that say which session, and therefore which developer,
-- is making an MCP call.
--
-- **Not `auth.session_tokens`**, which already exists in the same
-- SQLite file and means something else: that one is a human's login,
-- issued when a person signs in, and its subject is a user. This one
-- is an agent's working credential, issued when a work session is
-- created, and its subject is a session — the owner comes along
-- because a session has one. Different subjects, different lifetimes,
-- so a shared table would have to be nullable in both directions and
-- would answer neither question cleanly.
--
-- Before this, the MCP endpoint authenticated with one shared token
-- and every tool took `owner_id` as a parameter the caller asserted.
-- Anyone holding that token could read, list and delete another
-- developer's personal state by naming them — which is exactly what
-- the state model forbids ("not on the caller's honour").
--
-- Only the hash is stored. A token is shown once, when its session is
-- created, and cannot be recovered afterwards: a database that leaks
-- must not also hand over working credentials.
--
-- `owner_id` is denormalised from the session on purpose. Resolving a
-- caller happens on every single MCP request, and it must not depend
-- on a join to a row that may since have been deleted — a token whose
-- session is gone stops resolving, which is the behaviour wanted, and
-- a cascade delete makes that automatic.
CREATE TABLE IF NOT EXISTS session_agent_tokens (
    token_hash TEXT PRIMARY KEY,
    session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
    owner_id   TEXT NOT NULL,
    created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_session_agent_tokens_session
    ON session_agent_tokens(session_id);
