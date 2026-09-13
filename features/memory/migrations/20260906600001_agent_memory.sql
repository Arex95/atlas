-- Agent memory, carrying classification as a real column
-- rather than a convention. The CHECK is the point: a row that names
-- both a project and an owner, or neither, cannot be written at all
-- — not "is rejected by our code", which only holds until someone
-- adds a second write path.
--
-- Type 1 (project) memory belongs to a project and is shareable.
-- Type 2 (personal) memory belongs to a developer across *every*
-- project ("one developer working on any project"), which
-- is why there is no project column filled in for it.
CREATE TABLE agent_memory (
    id         TEXT NOT NULL PRIMARY KEY,
    state_type TEXT NOT NULL CHECK (state_type IN ('project', 'personal')),
    project    TEXT,
    owner_id   TEXT,
    key        TEXT NOT NULL,
    value      TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,

    CHECK (
        (state_type = 'project'  AND project  IS NOT NULL AND owner_id IS NULL)
     OR (state_type = 'personal' AND owner_id IS NOT NULL AND project  IS NULL)
    )
);

-- One value per key per bucket. Partial indexes rather than one
-- composite: the two buckets key on different columns, and a NULL in
-- a composite unique index would not collide the way we need.
CREATE UNIQUE INDEX idx_agent_memory_project_key
    ON agent_memory (project, key) WHERE state_type = 'project';
CREATE UNIQUE INDEX idx_agent_memory_owner_key
    ON agent_memory (owner_id, key) WHERE state_type = 'personal';
