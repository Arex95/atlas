//! Public surface of the `atlas-graph` crate.

pub use crate::internal::application::{
    Analyser, ChangedEntry, Coverage, Evidence, Finding, Findings, GraphWatcher,
    IMPACT_DEFAULT_DEPTH, ImpactAnalyser, ImpactEvidence, ImpactReport, Impacted, Indexer,
    LayerReport, Severity, Violation, WatchStatus,
};
pub use crate::internal::domain::{
    EdgePredicate, GraphEdge, GraphError, GraphNode, IndexStats, LAYERS_FILE, LayerDecl,
    LayersError, LayersFile, ModuleDecl, NodeKind,
};
pub use crate::internal::infrastructure::store::{Overview, Related};
pub use crate::internal::infrastructure::{ChangeKind, ChangedFile, GraphStore, changed_files};

pub use sqlx::SqlitePool;

/// # Errors
/// Any migration failure surfaced by sqlx.
pub async fn run_migrations(pool: &SqlitePool) -> Result<(), sqlx::migrate::MigrateError> {
    // Every feature crate migrates the same shared SQLite file into
    // one `_sqlx_migrations` table, so each migrator must be told to
    // ignore the migrations it did not resolve itself.
    let mut migrator = sqlx::migrate!("./migrations");
    migrator.set_ignore_missing(true);
    migrator.run(pool).await
}
