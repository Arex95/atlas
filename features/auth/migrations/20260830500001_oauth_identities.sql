-- Linkable OAuth identities against a single internal user
-- record. `access_token` is stored server-side deliberately — unlike
-- Mode 1 PATs (kept in the user's own config), the server
-- performs this OAuth dance itself and the resulting token becomes
-- available to the tracker adapter per "one grant covers
-- login and tracker reads."
CREATE TABLE oauth_identities (
    id               TEXT NOT NULL PRIMARY KEY,
    user_id          TEXT NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    provider         TEXT NOT NULL,
    provider_user_id TEXT NOT NULL,
    access_token     TEXT NOT NULL,
    scopes           TEXT NOT NULL,
    linked_at        TEXT NOT NULL,
    UNIQUE (provider, provider_user_id)
);

CREATE INDEX idx_oauth_identities_user ON oauth_identities (user_id);
