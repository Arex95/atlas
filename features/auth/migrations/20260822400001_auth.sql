-- Local-account identity for Mode 2 (local method only —
-- OAuth lands in separate issues). Single-tenant, self-hosted
--: no tenant column, and `users` bootstraps exactly once
-- (enforced in application code, not here — the schema has no
-- "is this the first row" concept of its own).
CREATE TABLE users (
    id            TEXT NOT NULL PRIMARY KEY,
    email         TEXT NOT NULL UNIQUE,
    password_hash TEXT NOT NULL,
    display_name  TEXT NOT NULL,
    created_at    TEXT NOT NULL,
    updated_at    TEXT NOT NULL
);

-- Opaque bearer tokens for authenticated HTTP calls (human/dashboard
-- auth — orthogonal to ATLAS_MCP_TOKEN, which gates the agent-facing
-- MCP endpoint). Only `token_hash` is stored; the raw token is
-- returned once, at creation, same custody principle as any other
-- credential in this project.
CREATE TABLE session_tokens (
    id         TEXT NOT NULL PRIMARY KEY,
    user_id    TEXT NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    token_hash TEXT NOT NULL UNIQUE,
    created_at TEXT NOT NULL,
    expires_at TEXT NOT NULL
);

CREATE INDEX idx_session_tokens_user ON session_tokens (user_id);
