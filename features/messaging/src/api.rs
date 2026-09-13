//! Public surface of the `atlas-messaging` crate.

pub use crate::internal::domain::{Message, MessageId, MessagingError, NewMessage};
pub use crate::internal::infrastructure::MessageStore;

pub use sqlx::SqlitePool;

/// # Errors
/// Any migration failure surfaced by sqlx.
pub async fn run_migrations(pool: &SqlitePool) -> Result<(), sqlx::migrate::MigrateError> {
    // Every feature crate migrates the same shared SQLite file into
    // one `_sqlx_migrations` table (modular monolith, one
    // database). Without `ignore_missing`, this migrator's own
    // `validate()` pass sees another feature's applied migrations —
    // versions it never resolved from its own `./migrations` dir —
    // and refuses to run at all (`VersionMissing`). Every feature's
    // `run_migrations` needs this, not just this one.
    let mut migrator = sqlx::migrate!("./migrations");
    migrator.set_ignore_missing(true);
    migrator.run(pool).await
}
