//! atlas-sync — `focus`-mode sync engine, first pilot on
//! sessions only, to begin with.
//!
//! The public surface is re-exported from [`api`]; consumers depend
//! only on that module (module boundary).

pub mod api;

mod internal;
