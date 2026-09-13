//! atlas-memory — agent memory, split into two buckets.
//!
//! What the agent knows about a *project* is Type 1 and shareable;
//! what it knows about a *person* is Type 2 and is not. That split is
//! the whole point of the crate, so it is carried by the schema and
//! by distinct store methods rather than by whoever happens to be
//! writing the query.
//!
//! The public surface is re-exported from [`api`]; consumers depend
//! only on that module (module boundary).

pub mod api;

mod internal;
