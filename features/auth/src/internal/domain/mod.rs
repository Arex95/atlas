//! The local-account identity vocabulary (local method).

mod error;
mod model;
mod oauth;
mod validation;

pub use error::AuthError;
pub use model::{InvitedAccount, IssuedSession, User, UserId};
pub use oauth::{OAuthProfile, OAuthProvider};
pub use validation::{validate_email, validate_password};

pub const SESSION_LIFETIME_DAYS: i64 = 30;
pub const MIN_PASSWORD_LEN: usize = 8;

/// How long a signed OAuth `state` value stays acceptable. Long
/// enough for a human to complete a provider's consent screen, short
/// enough that a leaked/logged callback URL isn't replayable for
/// long.
pub const OAUTH_STATE_MAX_AGE_SECS: i64 = 300;
