//! atlas-auth — local-account identity for Mode 2 (local
//! method only; OAuth lands in separate issues).
//!
//! The public surface is re-exported from [`api`]; consumers depend
//! only on that module (module boundary).

pub mod api;

mod internal;
