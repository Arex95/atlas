//! Minting and resolving session tokens.
//!
//! A token is 32 bytes from the OS random source, base64url-encoded
//! without padding so it survives a header, a URL and a shell
//! environment variable untouched.
//!
//! **Only the hash is stored.** The token is returned exactly once,
//! when its session is created, and is unrecoverable afterwards. A
//! database that leaks must not also hand over working credentials —
//! and this database is a file sitting next to the code it describes.
//!
//! SHA-256 rather than a password hash. The two are not
//! interchangeable and the difference is the input: a password is
//! low-entropy and chosen by a human, so Argon2 exists to make each
//! guess expensive. A 256-bit random token cannot be guessed at any
//! cost, and it is looked up on **every MCP request** — a deliberately
//! slow hash there would be a self-inflicted denial of service. Argon2
//! stays where it belongs, on passwords.

use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use rand::RngCore as _;
use sha2::{Digest, Sha256};

/// Bytes of entropy per token. 256 bits, the same order as the session
/// cookies of things that get audited.
const TOKEN_BYTES: usize = 32;

/// A freshly minted token, and the hash to store for it.
pub struct MintedToken {
    /// Shown to the caller once. Never stored.
    pub token: String,
    pub hash: String,
}

/// Generates a new token.
#[must_use]
pub fn mint() -> MintedToken {
    let mut bytes = [0u8; TOKEN_BYTES];
    // `OsRng` rather than a seeded generator: this is a credential, so
    // it must come from the operating system's entropy and never from
    // anything reproducible.
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    let token = URL_SAFE_NO_PAD.encode(bytes);
    let hash = hash(&token);
    MintedToken { token, hash }
}

/// The stored form of a token.
#[must_use]
pub fn hash(token: &str) -> String {
    let digest = Sha256::digest(token.as_bytes());
    // Hex rather than base64 so the column is unambiguous to read and
    // compares byte-for-byte in SQL.
    digest.iter().fold(String::with_capacity(64), |mut acc, b| {
        use std::fmt::Write as _;
        let _ = write!(acc, "{b:02x}");
        acc
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn a_token_is_url_and_shell_safe() {
        let minted = mint();
        assert!(
            minted
                .token
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'),
            "{} would need escaping somewhere",
            minted.token
        );
        assert!(!minted.token.contains('='), "padding survived");
    }

    #[test]
    fn two_tokens_are_never_the_same() {
        // A thousand is not proof, but a generator that repeats — a
        // seeded or time-based one — fails this immediately.
        let seen: HashSet<String> = (0..1000).map(|_| mint().token).collect();
        assert_eq!(seen.len(), 1000);
    }

    #[test]
    fn the_hash_is_stable_and_the_token_is_not_recoverable_from_it() {
        let minted = mint();
        assert_eq!(hash(&minted.token), minted.hash);
        assert_eq!(minted.hash.len(), 64);
        assert!(
            !minted.hash.contains(&minted.token),
            "the stored form contains the credential"
        );
    }

    #[test]
    fn a_different_token_hashes_differently() {
        assert_ne!(hash("one"), hash("two"));
    }

    #[test]
    fn the_token_carries_the_entropy_it_claims() {
        // 32 bytes base64url with no padding is 43 characters. A
        // shorter token means fewer bytes than intended reached it.
        assert_eq!(mint().token.len(), 43);
    }
}
