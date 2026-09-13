use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use sqlx::SqlitePool;
use thiserror::Error;
use url::Url;

use crate::internal::domain::{IssueTracker, ProjectRef};
use crate::internal::infrastructure::mirror::{MirrorStore, MirroredTracker};
use crate::internal::infrastructure::{DisabledTracker, GitLabTracker};

use super::runtime::TrackerRuntime;
use super::syncer::{MirrorSyncer, MirrorSyncerConfig};
use super::webhook_router::webhook_router;

/// Which adapter to build. `None` disables the feature at runtime;
/// every call returns [`TrackerError::Disabled`](crate::internal::domain::TrackerError::Disabled).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TrackerKind {
    None,
    GitLab,
}

impl TrackerKind {
    const ACCEPTED: &'static [&'static str] = &["none", "gitlab"];
}

impl std::str::FromStr for TrackerKind {
    type Err = TrackerConfigError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "none" => Ok(Self::None),
            "gitlab" => Ok(Self::GitLab),
            other => Err(TrackerConfigError::UnknownKind {
                got: other.to_owned(),
                accepted: Self::ACCEPTED,
            }),
        }
    }
}

/// Configuration for one Atlas instance. Built from environment
/// variables by the binary; the crate itself does not read env
/// (keeps composition pushed to the edge and tests trivial).
#[derive(Clone, Debug)]
pub struct TrackerConfig {
    pub kind: TrackerKind,
    /// Base URL of the tracker. Required when `kind == GitLab`.
    pub base_url: Option<Url>,
    /// Path to the file holding the PAT. Required when
    /// `kind == GitLab`. The file is read once at build time; the
    /// value is never persisted in Atlas.
    pub token_file: Option<PathBuf>,
    /// User agent Atlas sends on outgoing requests. Typically
    /// `atlas-server/<version>`.
    pub user_agent: String,
}

/// Configuration for the local mirror.
///
/// If `projects` is empty the syncer is not started; the mirror
/// tables still exist but stay empty. When populated, the mirror
/// wraps the upstream tracker so reads are local and outages of
/// the tracker do not fail reads.
#[derive(Clone, Debug)]
pub struct MirrorConfig {
    pub projects: Vec<ProjectRef>,
    pub interval: Duration,
}

#[derive(Debug, Error)]
pub enum TrackerConfigError {
    #[error("unknown tracker kind {got:?}; accepted values: {}", accepted.join(", "))]
    UnknownKind {
        got: String,
        accepted: &'static [&'static str],
    },

    #[error("tracker kind gitlab requires ATLAS_TRACKER_URL")]
    MissingBaseUrl,

    #[error("tracker kind gitlab requires ATLAS_TRACKER_TOKEN_FILE")]
    MissingTokenFile,

    #[error("could not read the tracker token file {path:?}: {source}")]
    TokenFileUnreadable {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("token file {path:?} is empty")]
    TokenFileEmpty { path: PathBuf },

    #[error("could not build the HTTP client for the tracker: {0}")]
    ClientBuild(String),
}

/// Build the raw upstream tracker (the real adapter or the
/// disabled no-op), unwrapped by the mirror. Callers that want the
/// mirror should use [`build_composition`] instead.
///
/// # Errors
/// Same conditions as [`build_composition`] for the upstream
/// portion: missing URL / token file / unreadable / empty.
pub fn build_upstream_tracker(
    config: &TrackerConfig,
) -> Result<Arc<dyn IssueTracker>, TrackerConfigError> {
    match config.kind {
        TrackerKind::None => Ok(Arc::new(DisabledTracker)),
        TrackerKind::GitLab => {
            let base_url = config
                .base_url
                .clone()
                .ok_or(TrackerConfigError::MissingBaseUrl)?;
            let token_path = config
                .token_file
                .clone()
                .ok_or(TrackerConfigError::MissingTokenFile)?;
            let token_raw = std::fs::read_to_string(&token_path).map_err(|source| {
                TrackerConfigError::TokenFileUnreadable {
                    path: token_path.clone(),
                    source,
                }
            })?;
            let token = token_raw.trim().to_owned();
            if token.is_empty() {
                return Err(TrackerConfigError::TokenFileEmpty { path: token_path });
            }
            Ok(Arc::new(
                GitLabTracker::new(base_url, token, config.user_agent.clone())
                    .map_err(TrackerConfigError::ClientBuild)?,
            ))
        }
    }
}

/// Assembled tracker composition. The syncer is *not* spawned —
/// the caller decides between `syncer.sync_once().await` (a single
/// pass, useful for a one-shot bootstrap) and `syncer.spawn()`
/// (the periodic loop, for a long-lived server).
pub struct Composition {
    pub runtime: TrackerRuntime,
    pub syncer: Option<MirrorSyncer>,
    /// Mounted at `/api/webhooks/tracker`. Empty unless the mirror is
    /// active *and* a secret is configured: with no mirror there is
    /// nothing to refresh, and with no secret the route would be an
    /// open way to make this server call the tracker on demand.
    pub webhook: axum::Router,
}

/// Build the tracker runtime plus, when applicable, the syncer.
/// The runtime points at the mirror if the mirror is active,
/// otherwise at the raw upstream.
///
/// The pool must already be migrated (call
/// [`crate::api::run_migrations`] first).
///
/// # Errors
/// See [`build_upstream_tracker`].
pub fn build_composition(
    tracker_config: &TrackerConfig,
    mirror_config: &MirrorConfig,
    pool: SqlitePool,
    webhook_secret: Option<String>,
) -> Result<Composition, TrackerConfigError> {
    let upstream = build_upstream_tracker(tracker_config)?;
    let store = MirrorStore::new(pool);

    let mirror_active =
        tracker_config.kind == TrackerKind::GitLab && !mirror_config.projects.is_empty();

    if mirror_active {
        let upstream_for_webhook = upstream.clone();
        let store_for_webhook = store.clone();
        let runtime = TrackerRuntime::new(Arc::new(MirroredTracker::new(
            store.clone(),
            upstream.clone(),
        )) as Arc<dyn IssueTracker>);
        let syncer = MirrorSyncer::new(MirrorSyncerConfig {
            upstream,
            store,
            projects: mirror_config.projects.clone(),
            interval: mirror_config.interval,
        });
        Ok(Composition {
            runtime,
            syncer: Some(syncer),
            webhook: webhook_router(upstream_for_webhook, store_for_webhook, webhook_secret),
        })
    } else {
        Ok(Composition {
            runtime: TrackerRuntime::new(upstream),
            syncer: None,
            webhook: axum::Router::new(),
        })
    }
}
