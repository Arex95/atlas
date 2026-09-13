//! The environment contract: every variable this server reads, what
//! it defaults to, and what makes it invalid.
//!
//! Split from `main.rs` because the two change for different reasons.
//! Composition changes when a feature crate is added or a route is
//! nested; this file changes when an operator-facing variable is
//! added, renamed, defaulted differently, or given a new rule. Adding
//! GitHub OAuth touched only this concern; adding workflow runs
//! touched only the other.
//!
//! `StartupError` lives here rather than beside `main` because every
//! variant interpolates one of these constants — leaving it behind is
//! what would couple the two halves back together.

use std::net::SocketAddr;
use std::path::PathBuf;
use std::str::FromStr;
use std::time::Duration;

use atlas_auth::api::{GithubOAuthConfig, GitlabOAuthConfig};
use atlas_tracker::api::{
    MirrorConfig, ProjectRef, ProjectRefError, TrackerConfig, TrackerConfigError, TrackerKind,
};
use url::Url;

pub(crate) const ENV_DB_PATH: &str = "ATLAS_DB_PATH";
pub(crate) const ENV_TRACKER_KIND: &str = "ATLAS_TRACKER_KIND";
pub(crate) const ENV_TRACKER_URL: &str = "ATLAS_TRACKER_URL";
pub(crate) const ENV_TRACKER_TOKEN_FILE: &str = "ATLAS_TRACKER_TOKEN_FILE";
pub(crate) const ENV_MIRROR_PROJECTS: &str = "ATLAS_TRACKER_MIRROR_PROJECTS";
pub(crate) const ENV_MIRROR_INTERVAL: &str = "ATLAS_TRACKER_MIRROR_INTERVAL_SECS";
pub(crate) const ENV_LISTEN_ADDR: &str = "ATLAS_LISTEN_ADDR";
pub(crate) const ENV_SERVER_URL: &str = "ATLAS_SERVER_URL";
pub(crate) const ENV_MCP_TOKEN: &str = "ATLAS_MCP_TOKEN";
pub(crate) const ENV_MCP_TOKEN_ALLOW_UNSET: &str = "ATLAS_MCP_TOKEN_ALLOW_UNSET";
pub(crate) const ENV_WORKSPACE_ROOT: &str = "ATLAS_WORKSPACE_ROOT";
pub(crate) const ENV_OAUTH_GITLAB_CLIENT_ID: &str = "ATLAS_OAUTH_GITLAB_CLIENT_ID";
pub(crate) const ENV_OAUTH_GITLAB_CLIENT_SECRET: &str = "ATLAS_OAUTH_GITLAB_CLIENT_SECRET";
pub(crate) const ENV_OAUTH_GITLAB_REDIRECT_URI: &str = "ATLAS_OAUTH_GITLAB_REDIRECT_URI";
pub(crate) const ENV_TRACKER_WEBHOOK_SECRET: &str = "ATLAS_TRACKER_WEBHOOK_SECRET";
pub(crate) const ENV_OAUTH_GITHUB_CLIENT_ID: &str = "ATLAS_OAUTH_GITHUB_CLIENT_ID";
pub(crate) const ENV_OAUTH_GITHUB_CLIENT_SECRET: &str = "ATLAS_OAUTH_GITHUB_CLIENT_SECRET";
pub(crate) const ENV_OAUTH_GITHUB_REDIRECT_URI: &str = "ATLAS_OAUTH_GITHUB_REDIRECT_URI";
pub(crate) const ENV_OAUTH_STATE_SECRET: &str = "ATLAS_OAUTH_STATE_SECRET";

pub(crate) const DEFAULT_DB_PATH: &str = "./atlas-data/atlas.db";
pub(crate) const DEFAULT_MIRROR_INTERVAL_SECS: u64 = 300;
pub(crate) const DEFAULT_LISTEN_ADDR: &str = "0.0.0.0:4000";
const DEV_ONLY_TOKEN: &str = "dev-token-unset";
pub(crate) const DEFAULT_WORKSPACE_ROOT: &str = "~/dev";

pub(crate) fn read_tracker_config() -> Result<TrackerConfig, StartupError> {
    let kind: TrackerKind = std::env::var(ENV_TRACKER_KIND)
        .unwrap_or_else(|_| "none".to_owned())
        .parse()?;

    let base_url = match std::env::var(ENV_TRACKER_URL) {
        Ok(raw) => Some(Url::parse(&raw).map_err(|e| StartupError::BadUrl(e.to_string()))?),
        Err(_) => None,
    };

    let token_file = std::env::var(ENV_TRACKER_TOKEN_FILE)
        .ok()
        .map(PathBuf::from);

    Ok(TrackerConfig {
        kind,
        base_url,
        token_file,
        user_agent: format!("atlas-server/{}", env!("CARGO_PKG_VERSION")),
    })
}

pub(crate) fn read_mirror_config() -> Result<MirrorConfig, StartupError> {
    let projects = std::env::var(ENV_MIRROR_PROJECTS)
        .ok()
        .map(|raw| {
            raw.split(',')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(ProjectRef::from_str)
                .collect::<Result<Vec<_>, _>>()
        })
        .transpose()
        .map_err(StartupError::MirrorProjects)?
        .unwrap_or_default();

    let interval_secs = match std::env::var(ENV_MIRROR_INTERVAL) {
        Ok(raw) => raw
            .parse::<u64>()
            .ok()
            .filter(|&n| n > 0)
            .ok_or(StartupError::BadMirrorInterval(raw))?,
        Err(_) => DEFAULT_MIRROR_INTERVAL_SECS,
    };

    Ok(MirrorConfig {
        projects,
        interval: Duration::from_secs(interval_secs),
    })
}

pub(crate) fn read_mcp_token() -> Result<String, StartupError> {
    if let Ok(raw) = std::env::var(ENV_MCP_TOKEN) {
        if raw.is_empty() {
            return Err(StartupError::EmptyMcpToken);
        }
        return Ok(raw);
    }
    let allow_unset = std::env::var(ENV_MCP_TOKEN_ALLOW_UNSET)
        .map(|v| v == "1")
        .unwrap_or(false);
    if allow_unset {
        tracing::warn!(
            "{ENV_MCP_TOKEN} is unset; falling back to a dev token because {ENV_MCP_TOKEN_ALLOW_UNSET}=1 — do NOT ship this"
        );
        Ok(DEV_ONLY_TOKEN.to_owned())
    } else {
        Err(StartupError::MissingMcpToken)
    }
}

pub(crate) fn read_workspace_root() -> Result<PathBuf, StartupError> {
    let raw =
        std::env::var(ENV_WORKSPACE_ROOT).unwrap_or_else(|_| DEFAULT_WORKSPACE_ROOT.to_owned());
    if let Some(rest) = raw.strip_prefix("~/") {
        let home = std::env::var("HOME").map_err(|_| StartupError::NoHome)?;
        return Ok(PathBuf::from(home).join(rest));
    }
    Ok(PathBuf::from(raw))
}

/// `None` when GitLab OAuth is simply not configured (the common
/// case — Mode 1 installs never set these). Partially set is a
/// configuration error, not a silent disable: someone meant to turn
/// this on and got it wrong.
pub(crate) fn read_gitlab_oauth_config() -> Result<Option<GitlabOAuthConfig>, StartupError> {
    let client_id = std::env::var(ENV_OAUTH_GITLAB_CLIENT_ID).ok();
    let client_secret = std::env::var(ENV_OAUTH_GITLAB_CLIENT_SECRET).ok();
    let redirect_uri = std::env::var(ENV_OAUTH_GITLAB_REDIRECT_URI).ok();
    let state_secret = std::env::var(ENV_OAUTH_STATE_SECRET).ok();

    match (client_id, client_secret, redirect_uri, state_secret) {
        (None, None, None, None) => Ok(None),
        (Some(client_id), Some(client_secret), Some(redirect_uri), Some(state_secret)) => Ok(Some(
            GitlabOAuthConfig::gitlab_com(client_id, client_secret, redirect_uri, state_secret),
        )),
        _ => Err(StartupError::IncompleteGitlabOAuthConfig),
    }
}

/// `None` when GitHub OAuth is not configured. Same all-or-nothing
/// rule as GitLab, and the same reason: a half-configured provider is
/// somebody's mistake, not a request to leave it off.
///
/// `ATLAS_OAUTH_STATE_SECRET` is shared with GitLab deliberately — it
/// signs the `state` value, which is about this server's own
/// round trip and not about which provider the round trip went to.
pub(crate) fn read_github_oauth_config() -> Result<Option<GithubOAuthConfig>, StartupError> {
    let client_id = std::env::var(ENV_OAUTH_GITHUB_CLIENT_ID).ok();
    let client_secret = std::env::var(ENV_OAUTH_GITHUB_CLIENT_SECRET).ok();
    let redirect_uri = std::env::var(ENV_OAUTH_GITHUB_REDIRECT_URI).ok();
    let state_secret = std::env::var(ENV_OAUTH_STATE_SECRET).ok();

    match (client_id, client_secret, redirect_uri, state_secret) {
        (None, None, None, _) => Ok(None),
        (Some(client_id), Some(client_secret), Some(redirect_uri), Some(state_secret)) => Ok(Some(
            GithubOAuthConfig::github_com(client_id, client_secret, redirect_uri, state_secret),
        )),
        _ => Err(StartupError::IncompleteGithubOAuthConfig),
    }
}

/// What a spawned terminal is told to call Atlas back on.
///
/// Derived from the listen address, which is right for the ordinary
/// case — a terminal runs on the same host as the server, so whatever
/// the server bound is reachable from it.
///
/// `0.0.0.0` is the exception and the reason this is overridable: it
/// means "every interface", not an address anything can connect to, so
/// it becomes loopback. A deployment where the terminal is somewhere
/// else entirely sets `ATLAS_SERVER_URL` and this guesses nothing.
pub(crate) fn read_server_url(listen_addr: SocketAddr) -> String {
    if let Ok(explicit) = std::env::var(ENV_SERVER_URL) {
        let trimmed = explicit.trim();
        if !trimmed.is_empty() {
            return trimmed.trim_end_matches('/').to_owned();
        }
    }
    let host = if listen_addr.ip().is_unspecified() {
        // Same host, so loopback is both reachable and the narrowest
        // thing that works.
        if listen_addr.is_ipv4() {
            "127.0.0.1".to_owned()
        } else {
            "[::1]".to_owned()
        }
    } else if listen_addr.is_ipv6() {
        format!("[{}]", listen_addr.ip())
    } else {
        listen_addr.ip().to_string()
    };
    format!("http://{host}:{}", listen_addr.port())
}

pub(crate) fn read_listen_addr() -> Result<SocketAddr, StartupError> {
    let raw = std::env::var(ENV_LISTEN_ADDR).unwrap_or_else(|_| DEFAULT_LISTEN_ADDR.to_owned());
    raw.parse::<SocketAddr>()
        .map_err(|e| StartupError::BadListenAddr(format!("{raw:?}: {e}")))
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum StartupError {
    #[error(transparent)]
    Config(#[from] TrackerConfigError),
    #[error("invalid {ENV_TRACKER_URL}: {0}")]
    BadUrl(String),
    #[error("could not create the database directory {path:?}: {source}")]
    DbDir {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("could not open the database: {0}")]
    DbOpen(#[source] sqlx::Error),
    #[error("database migration failed: {0}")]
    Migrate(#[source] sqlx::migrate::MigrateError),
    #[error("invalid entry in {ENV_MIRROR_PROJECTS}: {0}")]
    MirrorProjects(ProjectRefError),
    #[error("invalid {ENV_MIRROR_INTERVAL} (must be a positive integer): {0:?}")]
    BadMirrorInterval(String),
    #[error("invalid {ENV_LISTEN_ADDR}: {0}")]
    BadListenAddr(String),
    #[error(
        "{ENV_MCP_TOKEN} is required; set {ENV_MCP_TOKEN_ALLOW_UNSET}=1 to run without a token (dev only)"
    )]
    MissingMcpToken,
    #[error("{ENV_MCP_TOKEN} must not be empty")]
    EmptyMcpToken,
    #[error("{ENV_WORKSPACE_ROOT} used a ~/ prefix but $HOME is unset")]
    NoHome,
    #[error(
        "{ENV_OAUTH_GITLAB_CLIENT_ID}, {ENV_OAUTH_GITLAB_CLIENT_SECRET}, {ENV_OAUTH_GITLAB_REDIRECT_URI}, and {ENV_OAUTH_STATE_SECRET} must all be set together, or none of them (GitLab OAuth left disabled)"
    )]
    IncompleteGitlabOAuthConfig,
    #[error(
        "{ENV_OAUTH_GITHUB_CLIENT_ID}, {ENV_OAUTH_GITHUB_CLIENT_SECRET}, {ENV_OAUTH_GITHUB_REDIRECT_URI}, and {ENV_OAUTH_STATE_SECRET} must all be set together, or none of them (GitHub OAuth left disabled)"
    )]
    IncompleteGithubOAuthConfig,
    // Names the variable like every other startup error here does. This
    // is the one a developer actually hits — another Atlas, or anything
    // else, already on the port — so leaving it to say only "address in
    // use" made the most common failure the least actionable.
    #[error(
        "could not bind to {addr}: {source}. Set {ENV_LISTEN_ADDR} to a free address, e.g. 127.0.0.1:4001"
    )]
    Bind {
        addr: SocketAddr,
        #[source]
        source: std::io::Error,
    },
}
