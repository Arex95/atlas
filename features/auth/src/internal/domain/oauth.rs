//! The identity an OAuth provider hands back, and which provider it
//! came from.
//!
//! rule for every provider is the same and is the reason
//! this vocabulary is shared rather than duplicated per adapter:
//! **linking only, never creating.** A successful consent screen does
//! not make an account. It finds one that already exists and attaches
//! an identity to it, or it fails.
//!
//! That rule is what makes the email in [`OAuthProfile`] load-bearing,
//! because matching an existing account is done by it — see the
//! warning on that field.

use serde::Serialize;

/// A provider Atlas can link an identity from.
///
/// An enum rather than the string the database column stores, so a
/// typo cannot quietly invent a provider whose identities then match
/// nothing and whose rows nothing ever cleans up.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum OAuthProvider {
    Gitlab,
    Github,
}

impl OAuthProvider {
    /// The value stored in `oauth_identities.provider`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Gitlab => "gitlab",
            Self::Github => "github",
        }
    }
}

/// Who a provider says just authenticated.
pub struct OAuthProfile {
    /// The provider's own stable id for this account. What a returning
    /// login is matched on, because an email can change hands and this
    /// cannot.
    pub provider_user_id: String,

    /// **A verified address, or this is an account takeover.**
    ///
    /// A first login has no identity row yet, so it finds the account
    /// to link by matching this against `users.email`. An adapter that
    /// passes an address the provider has not confirmed lets anyone
    /// claim any account: add the victim's address to your own
    /// provider account, consent, and the link is made to theirs.
    ///
    /// The providers differ here and each adapter states which case it
    /// is in. GitLab's profile email is the account's confirmed
    /// primary. GitHub's is not necessarily verified at all, so that
    /// adapter asks a different endpoint.
    pub email: String,

    pub name: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_provider_has_its_own_stored_name() {
        // These strings are in the database and in a UNIQUE index. If
        // two ever collided, one provider's identities would match the
        // other's.
        assert_eq!(OAuthProvider::Gitlab.as_str(), "gitlab");
        assert_eq!(OAuthProvider::Github.as_str(), "github");
        assert_ne!(
            OAuthProvider::Gitlab.as_str(),
            OAuthProvider::Github.as_str()
        );
    }
}
