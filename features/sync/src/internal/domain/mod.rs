//! Sync engine vocabulary (`focus` slice 1, `auto` slice 2,
//! `live` slice 3 — the last over SSE).

mod error;
mod model;

pub use error::SyncError;
pub use model::{
    Audience, ChangeKind, LiveConfig, OwnerChange, SyncConfig, SyncMode, SyncReport,
    SyncStatusReport,
};
