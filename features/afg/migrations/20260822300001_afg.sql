-- Agent Flow Graph (AFG) — declarative workflows over the message bus
-- (slices 1+2 of the prototype). A workflow's spec is parsed from YAML
-- once at register time and stored as normalized JSON; the runtime
-- never re-parses the YAML on each run.
CREATE TABLE workflows (
    id          TEXT NOT NULL PRIMARY KEY,
    project     TEXT NOT NULL,
    name        TEXT NOT NULL,
    source_path TEXT,
    spec_json   TEXT NOT NULL,
    version     INTEGER NOT NULL DEFAULT 1,
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL,
    UNIQUE (project, name)
);

CREATE INDEX idx_workflows_project ON workflows (project);

-- `correlation_id` ties every message this run ever sends, so the
-- whole run's message trail is one `WHERE correlation_id = ?` away.
CREATE TABLE workflow_runs (
    id                    TEXT NOT NULL PRIMARY KEY,
    workflow_id           TEXT NOT NULL REFERENCES workflows (id) ON DELETE CASCADE,
    status                TEXT NOT NULL DEFAULT 'pending',
    current_node_id       TEXT,
    correlation_id        TEXT NOT NULL,
    initiator_session_id  TEXT NOT NULL,
    started_at            TEXT NOT NULL,
    updated_at            TEXT NOT NULL,
    completed_at          TEXT
);

CREATE INDEX idx_workflow_runs_workflow ON workflow_runs (workflow_id);
CREATE INDEX idx_workflow_runs_status ON workflow_runs (status);

-- The auditable timeline: `SELECT * FROM workflow_node_events WHERE
-- run_id = ? ORDER BY at` reconstructs a run's full history.
CREATE TABLE workflow_node_events (
    id           TEXT NOT NULL PRIMARY KEY,
    run_id       TEXT NOT NULL REFERENCES workflow_runs (id) ON DELETE CASCADE,
    node_id      TEXT NOT NULL,
    kind         TEXT NOT NULL,
    payload_json TEXT,
    message_id   TEXT,
    at           TEXT NOT NULL
);

CREATE INDEX idx_wne_run ON workflow_node_events (run_id, at);
