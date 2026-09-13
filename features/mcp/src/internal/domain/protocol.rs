//! Constants describing what this MCP endpoint is and speaks.
//!
//! The MCP protocol version is a single value: clients that
//! request a *newer* version get this one back and continue on
//! its shape, clients that request the exact match get it,
//! clients that request older strings we do not know about get
//! this one back too and it is on them to fall back.

pub const PROTOCOL_VERSION: &str = "2025-06-18";
pub const SERVER_NAME: &str = "atlas-server";
pub const SERVER_VERSION: &str = env!("CARGO_PKG_VERSION");
