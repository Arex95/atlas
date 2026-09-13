//! Who is making a call.
//!
//! The MCP endpoint used to authenticate with a single shared token
//! and then believe whatever `owner_id` the caller wrote in the
//! arguments. That is the shape the state model rules out —
//! personal state must not be reachable on the caller's honour — and
//! it was not theoretical: with only the shared token it was possible
//! to read, list and delete another developer's personal memory by
//! naming them.
//!
//! So identity stops being an argument and becomes a property of the
//! credential. There are exactly two credentials, and the difference
//! between them is the whole design:
//!
//! * A **session token**, minted when a session is created and
//!   injected into that session's terminal. It names a session, and
//!   through it an owner.
//! * The **shared MCP token**, which names no one. It resolves to
//!   [`LOCAL_OWNER`].
//!
//! There is deliberately **no mode branching** here. A standalone
//! install has one developer and no accounts, so everything it owns
//! is owned by `LOCAL_OWNER` and it keeps working with the shared
//! token alone. A team server issues real user ids, which
//! `LOCAL_OWNER` never collides with, so the shared token reaches
//! nobody's data there. One rule covers both, and a rule that does
//! not branch cannot be branched wrongly.

use serde::Serialize;

/// The owner a caller with no identity acts as.
///
/// Reserved: real user ids are ULIDs, so this cannot be one, and a
/// team server's users can never be impersonated by presenting the
/// shared token. On a standalone install it is simply *the* user.
pub const LOCAL_OWNER: &str = "local";

/// The identity behind one request.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Caller {
    /// Authenticated with the shared MCP token. Knows the client is
    /// permitted; does not know who it is.
    Local,
    /// Authenticated with a session token.
    Session {
        session_id: String,
        owner_id: String,
    },
}

impl Caller {
    /// The owner this caller may act as, and never any other.
    ///
    /// Every tool that touches personal state reads its owner from
    /// here rather than from its arguments. That is the whole fix:
    /// there is no longer a parameter to lie in.
    #[must_use]
    pub fn owner_id(&self) -> &str {
        match self {
            Self::Local => LOCAL_OWNER,
            Self::Session { owner_id, .. } => owner_id,
        }
    }

    /// The session this call came from, if it came from one.
    ///
    /// `None` for the shared token. A tool that needs to know which
    /// session is asking — rather than merely which developer — must
    /// treat that as a refusal, not as a default.
    #[must_use]
    pub fn session_id(&self) -> Option<&str> {
        match self {
            Self::Local => None,
            Self::Session { session_id, .. } => Some(session_id),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_shared_token_acts_as_the_reserved_local_owner() {
        assert_eq!(Caller::Local.owner_id(), LOCAL_OWNER);
        assert_eq!(Caller::Local.session_id(), None);
    }

    #[test]
    fn a_session_caller_carries_both_identities() {
        let caller = Caller::Session {
            session_id: "01SESSION".to_owned(),
            owner_id: "01USER".to_owned(),
        };
        assert_eq!(caller.owner_id(), "01USER");
        assert_eq!(caller.session_id(), Some("01SESSION"));
    }

    #[test]
    fn the_reserved_owner_cannot_be_a_real_user_id() {
        // Real ids are ULIDs: 26 characters, Crockford base32. If this
        // ever stopped holding, the shared token would start resolving
        // to somebody's account.
        let id = ulid::Ulid::new().to_string();
        assert_eq!(id.len(), 26);
        assert_ne!(id, LOCAL_OWNER);
        assert!(LOCAL_OWNER.len() < 26);
    }
}
