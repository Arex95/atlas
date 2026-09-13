//! Public surface of the `atlas-tracker` crate.
//!
//! Consumers depend on the re-exports here and never reach into
//! `crate::internal` — that's how module boundary is
//! actually enforced.

pub use crate::internal::application::{
    Composition, MirrorConfig, MirrorSyncer, MirrorSyncerConfig, TrackerConfig, TrackerConfigError,
    TrackerKind, TrackerRuntime, build_composition, build_upstream_tracker,
};
pub use crate::internal::domain::{
    AcceptanceCriteriaCount, Issue, IssueFilter, IssueId, IssuePlanProgress, IssueRelation,
    IssueStatus, IssueTracker, Label, NewIssue, PlanProgress, ProjectRef, ProjectRefError,
    TrackerError, aggregate_plan_progress, parse_acceptance_criteria,
};
pub use crate::internal::infrastructure::mirror::{MirrorStore, MirroredTracker};

pub use crate::internal::application::webhook_router;
#[cfg(any(test, feature = "test-support"))]
pub use crate::internal::infrastructure::{FakeTracker, GitLabTracker};

// SQLite pool + migrations helper. Callers open the pool through
// their own connection code and then call `run_migrations` before
// building the composition. The migration set is compiled into the
// crate binary via `sqlx::migrate!()`.
pub use sqlx::SqlitePool;

/// # Errors
/// Any migration failure surfaced by sqlx (bad SQL, checksum
/// mismatch after editing an already-applied migration, etc.).
pub async fn run_migrations(pool: &SqlitePool) -> Result<(), sqlx::migrate::MigrateError> {
    // `ignore_missing`: every feature crate migrates the same
    // shared SQLite file into one `_sqlx_migrations` table
    //. Without it this migrator's `validate()` pass sees
    // other features' applied migrations — versions it never
    // resolved from its own `./migrations` dir — and refuses to run
    // (`VersionMissing`). Every feature's `run_migrations` needs
    // this, not just this one.
    let mut migrator = sqlx::migrate!("./migrations");
    migrator.set_ignore_missing(true);
    migrator.run(pool).await
}
