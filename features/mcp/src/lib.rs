//! atlas-mcp — Model Context Protocol endpoint over HTTP.
//!
//! Exposes the tracker port (`atlas-tracker`) as JSON-RPC 2.0
//! tools an AI agent (Claude / Codex / Cursor / …) can call. This
//! crate provides the router and the state builder; the binary
//! mounts the router under `/api/mcp` and handles the HTTP bind,
//! auth token config and graceful shutdown.

pub mod api;

mod internal;
