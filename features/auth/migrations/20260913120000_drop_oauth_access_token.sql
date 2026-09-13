-- The stored OAuth access token, removed.
--
-- It was written on every login and read by nothing: a search of the
-- workspace found no query selecting it. The original migration
-- justified keeping it — the tracker adapter would use it one day —
-- and that consumer was never built. Until it is, the column was a
-- third party's live credential sitting in plaintext next to the
-- notes, earning nothing.
--
-- If the adapter is built, it comes back with its own design: where
-- the value lives, how it is rotated, what happens when it expires.
-- Those questions have answers; "we stored it just in case" is not one
-- of them.
--
-- Rebuilt rather than ALTER ... DROP COLUMN so this applies on every
-- SQLite the project supports, and so the surviving columns keep their
-- constraints.
CREATE TABLE oauth_identities_new (
    id               TEXT NOT NULL PRIMARY KEY,
    user_id          TEXT NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    provider         TEXT NOT NULL,
    provider_user_id TEXT NOT NULL,
    scopes           TEXT NOT NULL,
    linked_at        TEXT NOT NULL,
    UNIQUE (provider, provider_user_id)
);

INSERT INTO oauth_identities_new (id, user_id, provider, provider_user_id, scopes, linked_at)
SELECT id, user_id, provider, provider_user_id, scopes, linked_at FROM oauth_identities;

DROP TABLE oauth_identities;
ALTER TABLE oauth_identities_new RENAME TO oauth_identities;
