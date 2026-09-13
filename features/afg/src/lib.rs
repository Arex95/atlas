//! atlas-afg — declarative workflows over the message bus (AFG).
//!
//! The public surface is re-exported from [`api`]; consumers depend
//! only on that module (module boundary).

pub mod api;

mod internal;
