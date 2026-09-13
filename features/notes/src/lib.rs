//! atlas-notes — a developer's own notes (Type 2).
//!
//! Personal by construction rather than by classification: every note
//! has an owner, and there is no shared variant. A feature that asked
//! "share my scratch notes with the team" would have nothing to call.
//!
//! The public surface is re-exported from [`api`]; consumers depend
//! only on that module (module boundary).

pub mod api;

mod internal;
