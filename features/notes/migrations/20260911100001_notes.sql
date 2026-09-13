-- A developer's own notes (Type 2).
--
-- `owner_id` is NOT NULL and there is no other kind of note, so the
-- classification is carried by the shape rather than by a column
-- somebody has to set correctly: a row that belongs to nobody cannot
-- be written, and there is no "shared" variant to reach for by
-- accident.
--
-- Addressed by a name the developer chooses rather than by a generated
-- id, so writing one is idempotent and a terminal can reach it without
-- copying an id around: `atlas call notes.write name=todo ...`. A
-- scratchpad is simply the note called `scratchpad` — one concept, not
-- two tables for the same idea.
--
-- Not scoped to a project, deliberately. Personal state belongs
-- to "one developer working on *any* project", and a nullable project
-- column would make the uniqueness rule awkward in SQLite (NULLs do
-- not compare equal) for a distinction the name already expresses.
CREATE TABLE IF NOT EXISTS notes (
    id         TEXT NOT NULL PRIMARY KEY,
    owner_id   TEXT NOT NULL,
    name       TEXT NOT NULL,
    body       TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    UNIQUE (owner_id, name)
);

CREATE INDEX IF NOT EXISTS idx_notes_owner ON notes (owner_id, updated_at);
