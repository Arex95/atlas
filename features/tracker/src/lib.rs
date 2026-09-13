//! atlas-tracker — external issue tracker as a hexagonal port.
//!
//! The public surface is re-exported from [`api`]; consumers (the
//! entry-point binary, later features that need issue state) depend
//! only on that module. Everything under `internal` is an
//! implementation detail of this feature crate — do not import from
//! it across crates, or the crate boundary is not real.

pub mod api;

mod internal;

#[cfg(any(test, feature = "test-support"))]
pub mod contract;
