//! Public surface of the `atlas-notes` crate.

pub use crate::internal::domain::{Note, NotesError};
pub use crate::internal::infrastructure::NoteStore;

pub use sqlx::SqlitePool;

/// # Errors
/// Any migration failure surfaced by sqlx.
pub async fn run_migrations(pool: &SqlitePool) -> Result<(), sqlx::migrate::MigrateError> {
    // Every feature crate migrates the same shared SQLite file into
    // one `_sqlx_migrations` table; without this each
    // migrator's `validate()` rejects migrations it didn't resolve
    // itself.
    let mut migrator = sqlx::migrate!("./migrations");
    migrator.set_ignore_missing(true);
    migrator.run(pool).await
}
