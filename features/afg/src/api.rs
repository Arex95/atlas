//! Public surface of the `atlas-afg` crate.

pub use crate::internal::application::{AfgRuntime, router};
pub use crate::internal::domain::{
    AcceptanceCriterion, AfgError, NodeEventKind, RunDetail, RunId, RunStatus, Workflow,
    WorkflowId, WorkflowNodeEvent, WorkflowNodeSpec, WorkflowRun, WorkflowSpec, parse_spec,
};
pub use crate::internal::infrastructure::AfgStore;

pub use sqlx::SqlitePool;

/// # Errors
/// Any migration failure surfaced by sqlx.
pub async fn run_migrations(pool: &SqlitePool) -> Result<(), sqlx::migrate::MigrateError> {
    // See atlas-tracker's / atlas-messaging's `run_migrations` for
    // why `ignore_missing` is required: every feature crate migrates
    // the same shared SQLite file into one `_sqlx_migrations` table
    //, and without this each migrator's `validate()`
    // rejects migrations it didn't resolve itself.
    let mut migrator = sqlx::migrate!("./migrations");
    migrator.set_ignore_missing(true);
    migrator.run(pool).await
}
