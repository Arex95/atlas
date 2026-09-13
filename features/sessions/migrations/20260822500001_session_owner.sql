-- Sessions are personal state
-- and ownership is enforced "at the
-- repository layer, not on the caller's honour". `DEFAULT ''` here
-- is a SQLite `ALTER TABLE` requirement for a NOT NULL column, not a
-- real default — there is no pre-Mode-2 data to backfill, and every
-- `SessionStore::create` call requires a non-empty `owner_id`.
ALTER TABLE sessions ADD COLUMN owner_id TEXT NOT NULL DEFAULT '';

CREATE INDEX idx_sessions_owner ON sessions (owner_id);
