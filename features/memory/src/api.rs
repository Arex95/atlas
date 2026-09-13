//! Public surface of the `atlas-memory` crate.

pub use crate::internal::domain::{MemoryEntry, MemoryError, MemoryScope};
pub use crate::internal::infrastructure::MemoryStore;

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
