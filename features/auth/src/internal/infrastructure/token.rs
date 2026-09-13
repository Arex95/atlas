//! Opaque bearer session tokens.
//!
//! Unlike a password, a session token is already high-entropy
//! random data — a fast cryptographic hash (SHA-256) is the right
//! tool for looking it up in storage, not Argon2's deliberately-slow
//! KDF (which exists to blunt brute-forcing a low-entropy secret;
//! a token has no such weakness to defend against).

use std::fmt::Write as _;

use rand::RngCore;
use sha2::{Digest, Sha256};

const TOKEN_BYTES: usize = 32;

/// A fresh random token, hex-encoded. Long enough that guessing is
/// not a realistic attack.
#[must_use]
pub fn generate_token() -> String {
    let mut bytes = [0u8; TOKEN_BYTES];
    rand::thread_rng().fill_bytes(&mut bytes);
    hex_encode(&bytes)
}

#[must_use]
pub fn hash_token(token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    hex_encode(&hasher.finalize())
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        let _ = write!(out, "{b:02x}");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_tokens_are_unique_and_fixed_length() {
        let a = generate_token();
        let b = generate_token();
        assert_ne!(a, b);
        assert_eq!(a.len(), TOKEN_BYTES * 2);
    }

    #[test]
    fn hashing_is_deterministic() {
        let token = generate_token();
        assert_eq!(hash_token(&token), hash_token(&token));
    }
}
