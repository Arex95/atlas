-- Project Map: a content-centric, language-agnostic index of a project.
--
-- Nodes are pieces of content — a file, or a section of a markdown file.
-- Edges are typed relations between them. Nothing here is language-aware:
-- the same schema holds a Rust file, a README and a YAML config, which is
-- the point: see docs/concepts on why this is not a symbol graph.
--
-- `project` is a plain string, matching sessions, messaging and workflows.
-- There is no projects table in this server, and a foreign key to one
-- would be inventing a registry the rest of the system does without.

CREATE TABLE graph_nodes (
    id           TEXT NOT NULL PRIMARY KEY,
    project      TEXT NOT NULL,
    -- Canonical identity within a project: "src/auth.rs", or
    -- "docs/api.md#authentication" for a section. Edges resolve against
    -- this, so it has to be stable across reindexes of unchanged content.
    fqn          TEXT NOT NULL,
    name         TEXT NOT NULL,
    kind         TEXT NOT NULL CHECK (kind IN ('file', 'section')),
    extension    TEXT NOT NULL,
    file_path    TEXT NOT NULL,
    start_line   INTEGER NOT NULL,
    end_line     INTEGER NOT NULL,
    excerpt      TEXT,
    content_hash TEXT NOT NULL,
    created_at   TEXT NOT NULL,
    updated_at   TEXT NOT NULL,

    UNIQUE (project, fqn)
);

CREATE INDEX idx_graph_nodes_project_file ON graph_nodes (project, file_path);
CREATE INDEX idx_graph_nodes_project_kind ON graph_nodes (project, kind);

-- Typed directed relations.
--
-- `dst_id` is nullable on purpose. An edge whose target does not resolve to
-- a real node keeps its `dst_fqn` and stores NULL rather than pointing at a
-- guess — precision over recall. A wrong edge sends an agent somewhere
-- confidently, which is worse than an edge that is simply absent.
--
-- ON DELETE SET NULL rather than CASCADE for the destination: when a target
-- is re-extracted, the edge detaches instead of vanishing, so it can be
-- re-resolved without the source having to emit it again.
CREATE TABLE graph_edges (
    id         TEXT NOT NULL PRIMARY KEY,
    project    TEXT NOT NULL,
    src_id     TEXT NOT NULL REFERENCES graph_nodes (id) ON DELETE CASCADE,
    dst_id     TEXT REFERENCES graph_nodes (id) ON DELETE SET NULL,
    dst_fqn    TEXT NOT NULL,
    predicate  TEXT NOT NULL
        CHECK (predicate IN ('contains', 'links', 'mentions', 'imports')),
    file_path  TEXT NOT NULL,
    line       INTEGER NOT NULL,
    created_at TEXT NOT NULL
);

CREATE INDEX idx_graph_edges_project_src  ON graph_edges (project, src_id);
CREATE INDEX idx_graph_edges_project_dst  ON graph_edges (project, dst_id);
CREATE INDEX idx_graph_edges_project_pred ON graph_edges (project, predicate);

-- Per-file manifest, keyed by content hash. Not used by the full rebuild
-- this slice performs, but written by it: the watcher slice needs a record
-- of what was indexed and at which hash to decide what changed, and
-- backfilling it later would mean a reindex nobody asked for.
CREATE TABLE graph_files (
    project      TEXT NOT NULL,
    file_path    TEXT NOT NULL,
    content_hash TEXT NOT NULL,
    extension    TEXT NOT NULL,
    node_count   INTEGER NOT NULL DEFAULT 0,
    indexed_at   TEXT NOT NULL,

    PRIMARY KEY (project, file_path)
);

-- Full-text search over node names, identities and content.
-- `node_id` and `project` are UNINDEXED: they are carried to join back and
-- to scope results, never searched.
CREATE VIRTUAL TABLE graph_nodes_fts USING fts5(
    node_id UNINDEXED,
    project UNINDEXED,
    name,
    fqn,
    content
);
