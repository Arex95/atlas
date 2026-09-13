//! Public surface of the `atlas-mcp` crate.
//!
//! Consumers depend on the re-exports here and never reach into
//! `crate::internal` — the crate's module boundary.

pub use crate::internal::application::{McpState, router};
pub use crate::internal::infrastructure::McpToken;
