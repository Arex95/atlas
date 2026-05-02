PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS projects (
    id TEXT PRIMARY KEY,
    slug TEXT NOT NULL UNIQUE,
    name TEXT NOT NULL,
    description TEXT DEFAULT '',
    status TEXT NOT NULL DEFAULT 'active',
    root_path TEXT NOT NULL UNIQUE,
    index_path TEXT NOT NULL,
    deadline TEXT,
    tags TEXT DEFAULT '[]',
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    last_synced_at TEXT
);

CREATE UNIQUE INDEX IF NOT EXISTS projects_slug_idx ON projects(slug);
CREATE UNIQUE INDEX IF NOT EXISTS projects_root_path_idx ON projects(root_path);
CREATE INDEX IF NOT EXISTS projects_status_idx ON projects(status);
CREATE INDEX IF NOT EXISTS projects_created_at_idx ON projects(created_at);

CREATE TABLE IF NOT EXISTS ai_sessions (
    id TEXT PRIMARY KEY,
    project_id TEXT REFERENCES projects(id) ON DELETE SET NULL,
    provider TEXT NOT NULL,
    model TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'starting',
    pid INTEGER,
    pty_fd INTEGER,
    working_directory TEXT NOT NULL,
    prompt TEXT,
    mode TEXT NOT NULL DEFAULT 'interactive',
    linked_task_id TEXT,
    started_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    stopped_at TEXT,
    last_activity_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS ai_sessions_project_id_idx ON ai_sessions(project_id);
CREATE INDEX IF NOT EXISTS ai_sessions_status_idx ON ai_sessions(status);
CREATE INDEX IF NOT EXISTS ai_sessions_provider_idx ON ai_sessions(provider);
CREATE INDEX IF NOT EXISTS ai_sessions_linked_task_idx ON ai_sessions(linked_task_id);

CREATE TABLE IF NOT EXISTS agent_memory (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    key TEXT NOT NULL,
    value TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE UNIQUE INDEX IF NOT EXISTS agent_memory_project_key_idx ON agent_memory(project_id, key);

CREATE TABLE IF NOT EXISTS agent_skills (
    id TEXT PRIMARY KEY,
    project_id TEXT REFERENCES projects(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    description TEXT NOT NULL,
    script TEXT NOT NULL,
    usage_count INTEGER DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS agent_skills_project_id_idx ON agent_skills(project_id);
CREATE INDEX IF NOT EXISTS agent_skills_name_idx ON agent_skills(name);

CREATE TABLE IF NOT EXISTS notifications (
    id TEXT PRIMARY KEY,
    project_id TEXT REFERENCES projects(id) ON DELETE CASCADE,
    session_id TEXT REFERENCES ai_sessions(id) ON DELETE SET NULL,
    message TEXT NOT NULL,
    type TEXT NOT NULL DEFAULT 'info',
    status TEXT NOT NULL DEFAULT 'unread',
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS notifications_project_id_idx ON notifications(project_id);
CREATE INDEX IF NOT EXISTS notifications_session_id_idx ON notifications(session_id);
CREATE INDEX IF NOT EXISTS notifications_status_idx ON notifications(status);
CREATE INDEX IF NOT EXISTS notifications_created_at_idx ON notifications(created_at);
