//! Every SQL statement Project Map runs.

use chrono::{DateTime, Utc};
use sqlx::{Row, SqlitePool};

use crate::internal::domain::{
    EdgePredicate, FileFacts, GraphEdge, GraphError, GraphNode, LayersFile, ModuleDecl, NodeKind,
};

#[derive(Clone)]
pub struct GraphStore {
    pool: SqlitePool,
}

/// A node about to be written, with its id already assigned so edges
/// can point at it before it exists in the database.
pub struct NodeRow {
    pub id: String,
    pub fqn: String,
    pub name: String,
    pub kind: NodeKind,
    pub extension: String,
    pub file_path: String,
    pub start_line: i64,
    pub end_line: i64,
    pub excerpt: Option<String>,
    pub content_hash: String,
    pub search_text: String,
}

pub struct EdgeRow {
    pub src_id: String,
    pub dst_id: Option<String>,
    pub dst_fqn: String,
    pub predicate: EdgePredicate,
    pub file_path: String,
    pub line: i64,
}

pub struct FileRow {
    pub file_path: String,
    pub content_hash: String,
    pub extension: String,
    pub node_count: i64,
}

impl GraphStore {
    #[must_use]
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Replaces a project's entire graph in one transaction.
    ///
    /// All of it, atomically: a reader during a reindex sees the old
    /// graph or the new one, never a half-built one. That matters more
    /// here than the write cost, because the readers are agents making
    /// decisions from what they find.
    ///
    /// # Errors
    /// `Storage` on any SQL failure.
    pub async fn replace_project_graph(
        &self,
        project: &str,
        nodes: &[NodeRow],
        edges: &[EdgeRow],
        files: &[FileRow],
    ) -> Result<(), GraphError> {
        let now = Utc::now().to_rfc3339();
        let mut tx = self.pool.begin().await?;

        // FTS first: it has no foreign keys, and clearing it after the
        // nodes would leave a window where a search returns ids that
        // no longer exist.
        sqlx::query("DELETE FROM graph_nodes_fts WHERE project = ?")
            .bind(project)
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM graph_edges WHERE project = ?")
            .bind(project)
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM graph_nodes WHERE project = ?")
            .bind(project)
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM graph_files WHERE project = ?")
            .bind(project)
            .execute(&mut *tx)
            .await?;

        for node in nodes {
            sqlx::query(
                "INSERT INTO graph_nodes \
                 (id, project, fqn, name, kind, extension, file_path, start_line, end_line, \
                  excerpt, content_hash, created_at, updated_at) \
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(&node.id)
            .bind(project)
            .bind(&node.fqn)
            .bind(&node.name)
            .bind(node.kind.as_str())
            .bind(&node.extension)
            .bind(&node.file_path)
            .bind(node.start_line)
            .bind(node.end_line)
            .bind(&node.excerpt)
            .bind(&node.content_hash)
            .bind(&now)
            .bind(&now)
            .execute(&mut *tx)
            .await?;

            sqlx::query(
                "INSERT INTO graph_nodes_fts (node_id, project, name, fqn, content) \
                 VALUES (?, ?, ?, ?, ?)",
            )
            .bind(&node.id)
            .bind(project)
            .bind(&node.name)
            .bind(&node.fqn)
            .bind(&node.search_text)
            .execute(&mut *tx)
            .await?;
        }

        for edge in edges {
            sqlx::query(
                "INSERT INTO graph_edges \
                 (id, project, src_id, dst_id, dst_fqn, predicate, file_path, line, created_at) \
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(ulid::Ulid::new().to_string())
            .bind(project)
            .bind(&edge.src_id)
            .bind(&edge.dst_id)
            .bind(&edge.dst_fqn)
            .bind(edge.predicate.as_str())
            .bind(&edge.file_path)
            .bind(edge.line)
            .bind(&now)
            .execute(&mut *tx)
            .await?;
        }

        for file in files {
            sqlx::query(
                "INSERT INTO graph_files \
                 (project, file_path, content_hash, extension, node_count, indexed_at) \
                 VALUES (?, ?, ?, ?, ?, ?)",
            )
            .bind(project)
            .bind(&file.file_path)
            .bind(&file.content_hash)
            .bind(&file.extension)
            .bind(file.node_count)
            .bind(&now)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(())
    }

    /// Counts per node kind and per edge predicate, plus the files
    /// nothing points at.
    ///
    /// # Errors
    /// `Storage` on any SQL failure.
    pub async fn overview(&self, project: &str) -> Result<Overview, GraphError> {
        let files: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM graph_nodes WHERE project = ? AND kind = 'file'",
        )
        .bind(project)
        .fetch_one(&self.pool)
        .await?;
        let sections: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM graph_nodes WHERE project = ? AND kind = 'section'",
        )
        .bind(project)
        .fetch_one(&self.pool)
        .await?;

        let edge_rows = sqlx::query(
            "SELECT predicate, COUNT(*) AS n FROM graph_edges WHERE project = ? \
             GROUP BY predicate ORDER BY predicate",
        )
        .bind(project)
        .fetch_all(&self.pool)
        .await?;
        let edges_by_predicate = edge_rows
            .iter()
            .map(|r| (r.get::<String, _>("predicate"), r.get::<i64, _>("n")))
            .collect();

        let unresolved: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM graph_edges WHERE project = ? AND dst_id IS NULL",
        )
        .bind(project)
        .fetch_one(&self.pool)
        .await?;

        // Most-pointed-at files: where an agent should look first.
        let hub_rows = sqlx::query(
            "SELECT n.fqn AS fqn, COUNT(e.id) AS n \
             FROM graph_nodes n JOIN graph_edges e ON e.dst_id = n.id \
             WHERE n.project = ? AND n.kind = 'file' \
             GROUP BY n.id ORDER BY n DESC, n.fqn LIMIT 10",
        )
        .bind(project)
        .fetch_all(&self.pool)
        .await?;
        let hubs = hub_rows
            .iter()
            .map(|r| (r.get::<String, _>("fqn"), r.get::<i64, _>("n")))
            .collect();

        // Files nothing points at and which point nowhere: often dead,
        // sometimes just entry points.
        let orphans: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM graph_nodes n WHERE n.project = ? AND n.kind = 'file' \
             AND NOT EXISTS (SELECT 1 FROM graph_edges e WHERE e.dst_id = n.id) \
             AND NOT EXISTS (SELECT 1 FROM graph_edges e WHERE e.src_id = n.id)",
        )
        .bind(project)
        .fetch_one(&self.pool)
        .await?;

        Ok(Overview {
            files,
            sections,
            edges_by_predicate,
            edges_unresolved: unresolved,
            hubs,
            orphan_files: orphans,
            // Read from the same snapshot as everything else here, so
            // the modules an agent is offered are the ones that were
            // declared when the graph was built.
            modules: self.declared_modules(project).await?,
        })
    }

    /// Full-text search, newest-matching first.
    ///
    /// # Errors
    /// `Storage` on any SQL failure. A query FTS5 cannot parse comes
    /// back as an empty result rather than an error — a search that
    /// finds nothing is a normal answer.
    pub async fn find(
        &self,
        project: &str,
        query: &str,
        limit: i64,
    ) -> Result<Vec<GraphNode>, GraphError> {
        let rows = sqlx::query(
            "SELECT n.* FROM graph_nodes_fts f \
             JOIN graph_nodes n ON n.id = f.node_id \
             WHERE f.project = ? AND graph_nodes_fts MATCH ? \
             ORDER BY rank LIMIT ?",
        )
        .bind(project)
        .bind(query)
        .bind(limit)
        .fetch_all(&self.pool)
        .await;

        match rows {
            Ok(rows) => rows.iter().map(node_from_row).collect(),
            // FTS5 rejects malformed queries at execution. An agent
            // searching for `foo(` should get no results, not a 500.
            Err(sqlx::Error::Database(_)) => Ok(Vec::new()),
            Err(other) => Err(other.into()),
        }
    }

    /// # Errors
    /// `NotFound` if the project has no node with that identity,
    /// `Storage` on any SQL failure.
    pub async fn node(&self, project: &str, fqn: &str) -> Result<GraphNode, GraphError> {
        let row = sqlx::query("SELECT * FROM graph_nodes WHERE project = ? AND fqn = ?")
            .bind(project)
            .bind(fqn)
            .fetch_optional(&self.pool)
            .await?
            .ok_or(GraphError::NotFound)?;
        node_from_row(&row)
    }

    /// Every edge touching a node, in both directions.
    ///
    /// # Errors
    /// `NotFound` if there is no such node, `Storage` on any SQL
    /// failure.
    pub async fn related(&self, project: &str, fqn: &str) -> Result<Related, GraphError> {
        let node = self.node(project, fqn).await?;

        let outgoing = sqlx::query(
            "SELECT e.*, t.fqn AS resolved_fqn FROM graph_edges e \
             LEFT JOIN graph_nodes t ON t.id = e.dst_id \
             WHERE e.project = ? AND e.src_id = ? ORDER BY e.predicate, e.dst_fqn",
        )
        .bind(project)
        .bind(&node.id)
        .fetch_all(&self.pool)
        .await?;

        let incoming = sqlx::query(
            "SELECT e.*, s.fqn AS resolved_fqn FROM graph_edges e \
             JOIN graph_nodes s ON s.id = e.src_id \
             WHERE e.project = ? AND e.dst_id = ? ORDER BY e.predicate, s.fqn",
        )
        .bind(project)
        .bind(&node.id)
        .fetch_all(&self.pool)
        .await?;

        Ok(Related {
            node,
            outgoing: outgoing
                .iter()
                .map(edge_from_row)
                .collect::<Result<_, _>>()?,
            incoming: incoming
                .iter()
                .map(edge_from_row)
                .collect::<Result<_, _>>()?,
        })
    }

    /// Every file node, reduced to what the analyser needs.
    ///
    /// # Errors
    /// `Storage` on any SQL failure.
    pub async fn file_nodes(&self, project: &str) -> Result<Vec<FileFacts>, GraphError> {
        let rows = sqlx::query(
            "SELECT id, fqn, end_line FROM graph_nodes \
             WHERE project = ? AND kind = 'file' ORDER BY fqn",
        )
        .bind(project)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .iter()
            .map(|r| FileFacts {
                id: r.get("id"),
                fqn: r.get("fqn"),
                lines: r.get("end_line"),
            })
            .collect())
    }

    /// Import edges that resolved to a real node, as `(src_id, dst_id)`.
    ///
    /// Only these count as dependencies. `mentions` is heuristic,
    /// `links` is documentation, `contains` is structure — none of
    /// them means "this file needs that file", and computing an
    /// architecture finding from them would launder a guess into a
    /// claim.
    ///
    /// # Errors
    /// `Storage` on any SQL failure.
    pub async fn resolved_imports(
        &self,
        project: &str,
    ) -> Result<Vec<(String, String, Option<i64>)>, GraphError> {
        let rows = sqlx::query(
            "SELECT src_id, dst_id, line FROM graph_edges \
             WHERE project = ? AND predicate = 'imports' AND dst_id IS NOT NULL",
        )
        .bind(project)
        .fetch_all(&self.pool)
        .await?;

        // The line comes along because a layer violation has to point
        // at the import statement itself. A finding a reader has to go
        // and search for is one they will not act on.
        Ok(rows
            .iter()
            .map(|r| (r.get("src_id"), r.get("dst_id"), r.get("line")))
            .collect())
    }

    /// Import edges naming something outside the project, or nothing.
    /// Counted so a report can publish the ratio rather than imply it.
    ///
    /// # Errors
    /// `Storage` on any SQL failure.
    pub async fn unresolved_import_count(&self, project: &str) -> Result<usize, GraphError> {
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM graph_edges \
             WHERE project = ? AND predicate = 'imports' AND dst_id IS NULL",
        )
        .bind(project)
        .fetch_one(&self.pool)
        .await?;
        Ok(usize::try_from(count).unwrap_or(0))
    }

    /// The subdivisions a project declared, or none.
    ///
    /// A declaration that no longer parses yields none rather than an
    /// error: the caller asked for an overview, and failing the whole
    /// overview over a typo in an optional file would be the wrong
    /// trade. `graph.findings` is where a broken declaration is
    /// reported.
    async fn declared_modules(&self, project: &str) -> Result<Vec<ModuleDecl>, GraphError> {
        let (source, _) = self.project_layers(project).await?;
        Ok(source
            .and_then(|s| LayersFile::parse(&s).ok())
            .map(|f| f.modules)
            .unwrap_or_default())
    }

    /// The content hash of every indexed file, by path.
    ///
    /// Used to tell whether the graph still describes what is on disk.
    /// An impact report computed from a graph that predates the edit
    /// is not wrong so much as answering a slightly older question,
    /// and the reader has to be told which.
    ///
    /// # Errors
    /// `Storage` on any SQL failure.
    pub async fn file_hashes(
        &self,
        project: &str,
    ) -> Result<std::collections::HashMap<String, String>, GraphError> {
        let rows: Vec<(String, String)> =
            sqlx::query_as("SELECT file_path, content_hash FROM graph_files WHERE project = ?")
                .bind(project)
                .fetch_all(&self.pool)
                .await?;
        Ok(rows.into_iter().collect())
    }

    /// Records what a project declared about its own structure.
    ///
    /// Written on every index, so the declaration in the database is
    /// always the one that was on disk when the graph was built. A
    /// project that deletes its declaration file gets NULL here rather
    /// than a stale rule it is still being judged against.
    ///
    /// # Errors
    /// `Storage` on any SQL failure.
    pub async fn replace_project_layers(
        &self,
        project: &str,
        source: Option<&str>,
        parse_error: Option<&str>,
    ) -> Result<(), GraphError> {
        sqlx::query(
            "INSERT INTO graph_layers (project, source, parse_error, updated_at)              VALUES (?, ?, ?, ?)              ON CONFLICT(project) DO UPDATE SET                source = excluded.source,                parse_error = excluded.parse_error,                updated_at = excluded.updated_at",
        )
        .bind(project)
        .bind(source)
        .bind(parse_error)
        .bind(Utc::now().to_rfc3339())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// What a project declared, as (source, `parse_error`).
    ///
    /// Both are `None` for a project with no declaration file, which is
    /// the ordinary case and not an error.
    ///
    /// # Errors
    /// `Storage` on any SQL failure.
    pub async fn project_layers(
        &self,
        project: &str,
    ) -> Result<(Option<String>, Option<String>), GraphError> {
        let row: Option<(Option<String>, Option<String>)> =
            sqlx::query_as("SELECT source, parse_error FROM graph_layers WHERE project = ?")
                .bind(project)
                .fetch_optional(&self.pool)
                .await?;
        Ok(row.unwrap_or((None, None)))
    }

    /// A markdown file's headings, in document order.
    ///
    /// # Errors
    /// `Storage` on any SQL failure.
    pub async fn outline(
        &self,
        project: &str,
        file_path: &str,
    ) -> Result<Vec<GraphNode>, GraphError> {
        let rows = sqlx::query(
            "SELECT * FROM graph_nodes \
             WHERE project = ? AND file_path = ? AND kind = 'section' \
             ORDER BY start_line",
        )
        .bind(project)
        .bind(file_path)
        .fetch_all(&self.pool)
        .await?;
        rows.iter().map(node_from_row).collect()
    }
}

#[derive(Debug)]
pub struct Overview {
    pub files: i64,
    pub sections: i64,
    pub edges_by_predicate: Vec<(String, i64)>,
    pub edges_unresolved: i64,
    pub hubs: Vec<(String, i64)>,
    pub orphan_files: i64,
    /// What the project declared as its own subdivisions. Empty when
    /// it declared none — every directory is a candidate subdivision,
    /// and a list of all of them is a directory listing, not a map.
    pub modules: Vec<ModuleDecl>,
}

#[derive(Debug)]
pub struct Related {
    pub node: GraphNode,
    pub outgoing: Vec<GraphEdge>,
    pub incoming: Vec<GraphEdge>,
}

fn node_from_row(row: &sqlx::sqlite::SqliteRow) -> Result<GraphNode, GraphError> {
    let raw_kind: String = row.get("kind");
    Ok(GraphNode {
        id: row.get("id"),
        project: row.get("project"),
        fqn: row.get("fqn"),
        name: row.get("name"),
        kind: NodeKind::parse(&raw_kind)
            .ok_or_else(|| GraphError::Storage(format!("unknown node kind {raw_kind:?}")))?,
        extension: row.get("extension"),
        file_path: row.get("file_path"),
        start_line: row.get("start_line"),
        end_line: row.get("end_line"),
        excerpt: row.get("excerpt"),
        content_hash: row.get("content_hash"),
        created_at: parse_ts(&row.get::<String, _>("created_at"))?,
        updated_at: parse_ts(&row.get::<String, _>("updated_at"))?,
    })
}

fn edge_from_row(row: &sqlx::sqlite::SqliteRow) -> Result<GraphEdge, GraphError> {
    let predicate: String = row.get("predicate");
    Ok(GraphEdge {
        id: row.get("id"),
        project: row.get("project"),
        src_id: row.get("src_id"),
        dst_id: row.get("dst_id"),
        dst_fqn: row.get("dst_fqn"),
        predicate: EdgePredicate::parse(&predicate)
            .ok_or_else(|| GraphError::Storage(format!("unknown predicate {predicate:?}")))?,
        file_path: row.get("file_path"),
        line: row.get("line"),
    })
}

fn parse_ts(s: &str) -> Result<DateTime<Utc>, GraphError> {
    DateTime::parse_from_rfc3339(s)
        .map(|dt| dt.with_timezone(&Utc))
        .map_err(|e| GraphError::Storage(format!("stored timestamp unparsable: {e}")))
}
