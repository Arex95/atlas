//! atlas-terminal — spawns a real PTY bound to a registered session.
//!
//! The public surface is re-exported from [`api`]; consumers depend
//! only on that module (module boundary).

pub mod api;

mod internal;
