//! Where a driver's failure becomes this crate's error.
//!
//! The conversion lives beside the adapter that produces it rather
//! than in the domain, because an `impl From<sqlx::Error>` in
//! `domain/` makes the domain import `sqlx` — and a domain that knows
//! the name of its database driver cannot have that driver replaced
//! without touching the rules. See `features/README.md`.

use crate::internal::domain::MemoryError;

impl From<sqlx::Error> for MemoryError {
    fn from(e: sqlx::Error) -> Self {
        Self::Storage(e.to_string())
    }
}
