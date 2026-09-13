-- Mirror of the external tracker.
-- Owned by the `tracker` feature (repo CLAUDE.md: migrations live
-- inside the feature that owns the schema).

CREATE TABLE mirror_issues (
    project     TEXT NOT NULL,
    issue_id    TEXT NOT NULL,
    title       TEXT NOT NULL,
    status      TEXT NOT NULL,
    labels_json TEXT NOT NULL,
    author      TEXT NOT NULL,
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL,
    fetched_at  TEXT NOT NULL,
    PRIMARY KEY (project, issue_id)
);

CREATE INDEX idx_mirror_issues_status
    ON mirror_issues (project, status);
CREATE INDEX idx_mirror_issues_updated_at
    ON mirror_issues (project, updated_at);

CREATE TABLE mirror_relations (
    project    TEXT NOT NULL,
    issue_id   TEXT NOT NULL,
    kind       TEXT NOT NULL,
    target_id  TEXT NOT NULL,
    PRIMARY KEY (project, issue_id, kind, target_id)
);

-- Per-project cursor for delta pulls (last successful sync).
-- One row per project mirrored.
CREATE TABLE mirror_sync_cursor (
    project              TEXT NOT NULL PRIMARY KEY,
    last_successful_sync TEXT NOT NULL
);
