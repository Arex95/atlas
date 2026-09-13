//! Local read-mostly mirror of the external tracker.
//!
//! Two things live here:
//! - [`MirrorStore`] — `SQLite` persistence, all the SQL lives here
//!   and nothing else in the crate touches sqlx directly.
//! - [`MirroredTracker`] — implements [`IssueTracker`] by reading
//!   from the store; makes the mirror interchangeable with any
//!   other adapter through the same port.
//!
//! Migrations that own these tables live at
//! `features/tracker/migrations/`.
//!
//! [`IssueTracker`]: crate::internal::domain::IssueTracker

mod mapping;
mod store;
mod tracker;

pub use store::MirrorStore;
pub use tracker::MirroredTracker;
