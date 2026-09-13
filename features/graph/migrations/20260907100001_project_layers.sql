-- What a project declared about its own structure, captured when it
-- was last indexed.
--
-- Stored rather than read on demand so that findings never touch the
-- filesystem: the analyser answers from the same snapshot the rest of
-- the graph was built from, and the watcher keeps it current for free
-- because a change to the declaration file is a change to a file.
--
-- `parse_error` holds the reason a declaration was rejected. A broken
-- declaration must not fail indexing — the graph is still worth having
-- — but it must not be silently ignored either, so it is kept here and
-- reported as a finding.
CREATE TABLE IF NOT EXISTS graph_layers (
    project     TEXT PRIMARY KEY,
    -- The file as it was on disk, or NULL when the project has none.
    source      TEXT,
    parse_error TEXT,
    updated_at  TEXT NOT NULL
);
