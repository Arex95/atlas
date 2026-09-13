-- Session registry . `relative_path` is relative
-- to the machine's own `workspace_root` — never an absolute path
-- (that was the prototype's mistake). Resolving it against
-- `workspace_root` into something usable on disk is the PTY-spawn
-- feature's job, not this one's.
--
-- Type 2 personal state — replicates private-to-owner
-- once Mode 2 exists; no sync engine consumes that
-- classification yet.
CREATE TABLE sessions (
    id            TEXT NOT NULL PRIMARY KEY,
    project       TEXT NOT NULL,
    remote_url    TEXT NOT NULL,
    branch        TEXT NOT NULL,
    relative_path TEXT NOT NULL,
    agent_kind    TEXT NOT NULL DEFAULT 'bash',
    title         TEXT,
    status        TEXT NOT NULL DEFAULT 'active',
    created_at    TEXT NOT NULL,
    updated_at    TEXT NOT NULL
);

CREATE INDEX idx_sessions_project_status ON sessions (project, status);
