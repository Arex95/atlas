use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct UserId(pub String);

/// Never carries `password_hash` — that field exists only inside
/// the store, never in a type handed back across the crate
/// boundary.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct User {
    pub id: UserId,
    pub email: String,
    pub display_name: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    /// Set on an invited account until its server-generated
    /// temporary password is replaced. A caller with this set can
    /// only reach `/me` and `/change-password` — see
    /// `AuthError::PasswordChangeRequired`.
    pub must_change_password: bool,
}

/// The result of inviting a new account — never a session, since the
/// inviter isn't logging in as the invitee. `temporary_password` is
/// the raw value, returned exactly once, same custody principle as a
/// session token: only its hash is ever persisted.
#[derive(Clone, Debug)]
pub struct InvitedAccount {
    pub user: User,
    pub temporary_password: String,
}

/// A freshly issued session. `token` is the raw bearer value —
/// returned exactly once, at creation. Nothing after this point
/// (including this crate's own storage) ever sees it again; only
/// its hash is persisted.
#[derive(Clone, Debug)]
pub struct IssuedSession {
    pub token: String,
    pub user: User,
    pub expires_at: DateTime<Utc>,
}
