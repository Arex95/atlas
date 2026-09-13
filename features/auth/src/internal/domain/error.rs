use thiserror::Error;

#[derive(Debug, Error)]
pub enum AuthError {
    /// `register` after the first user already exists. Never
    /// distinguishes "email taken" from "already bootstrapped" —
    /// both collapse to this, since after bootstrap the endpoint is
    /// simply closed, not a general signup with validation errors.
    #[error("this server already has an account; registration is closed")]
    AlreadyBootstrapped,

    #[error("email must not be empty and must contain '@'")]
    InvalidEmail,

    #[error("password must be at least 8 characters")]
    PasswordTooShort,

    /// Deliberately covers both "no such email" and "wrong
    /// password" — the caller must not learn which one it was.
    #[error("invalid credentials")]
    InvalidCredentials,

    /// Covers both "no such token" and "token expired" for the same
    /// reason — an attacker probing session tokens learns nothing
    /// extra from the failure mode.
    #[error("invalid or expired session")]
    InvalidSession,

    #[error("storage failure: {0}")]
    Storage(String),

    /// Covers a missing, malformed, expired, or signature-mismatched
    /// `state` parameter — deliberately one variant, since a caller
    /// probing the callback learns nothing from which specific
    /// reason it failed.
    #[error("invalid or expired oauth state")]
    OAuthStateInvalid,

    /// The provider's token or profile endpoint failed, or returned
    /// something this code couldn't use (network failure, non-2xx
    /// response, malformed body).
    #[error("oauth provider error: {0}")]
    OAuthProviderError(String),

    /// GitLab OAuth found no local account to log into and — per
    /// Linking never creates an account on this path. Also returned when
    /// the matched account is disabled, deliberately the same as "no
    /// such account": a caller probing this endpoint learns nothing
    /// about which case it was.
    #[error("no local account is available for this GitLab identity")]
    OAuthAccountNotLinked,

    /// `disable_user`/`enable_user` targeting an id that doesn't
    /// exist.
    #[error("no such user")]
    UserNotFound,

    /// A caller tried to disable the account they're authenticated
    /// as, which would lock out the only session that could undo it.
    #[error("cannot disable your own account")]
    CannotDisableSelf,

    /// `invite` targeting an email that already has an account —
    /// unlike GitLab OAuth's link step, an invite is meant to create
    /// a brand new account, so a collision is a caller mistake worth
    /// surfacing rather than silently reusing the existing one.
    #[error("this email already has an account")]
    EmailAlreadyRegistered,

    /// The caller's account still carries a server-generated
    /// temporary password (from `invite`) that hasn't been replaced
    /// yet. Every route except `/me` and `/change-password` rejects
    /// with this until it is.
    #[error("a password change is required before this action")]
    PasswordChangeRequired,
}
