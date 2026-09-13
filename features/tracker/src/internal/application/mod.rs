//! Application layer — thin composition on top of the domain port.

mod factory;
mod runtime;
mod syncer;
pub mod webhook;
mod webhook_router;

pub use factory::{
    Composition, MirrorConfig, TrackerConfig, TrackerConfigError, TrackerKind, build_composition,
    build_upstream_tracker,
};
pub use runtime::TrackerRuntime;
pub use syncer::{MirrorSyncer, MirrorSyncerConfig};
pub use webhook_router::webhook_router;
