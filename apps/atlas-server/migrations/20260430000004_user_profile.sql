CREATE TABLE IF NOT EXISTS user_profile (
    id TEXT PRIMARY KEY DEFAULT 'default',
    name TEXT NOT NULL DEFAULT 'Developer',
    title TEXT NOT NULL DEFAULT 'Atlas User',
    email TEXT NOT NULL DEFAULT '',
    github TEXT NOT NULL DEFAULT '',
    website TEXT NOT NULL DEFAULT '',
    avatar_color TEXT NOT NULL DEFAULT '#3b82f6',
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

INSERT OR IGNORE INTO user_profile (id) VALUES ('default');
