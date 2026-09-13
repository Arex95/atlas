//! Argon2id password hashing. Uses the crate's own recommended
//! default parameters — no hand-rolled cost tuning without a
//! stated reason to deviate from them.

use argon2::Argon2;
use argon2::password_hash::rand_core::OsRng;
use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};

/// # Errors
/// If the underlying Argon2 hashing call fails (should not happen
/// under normal conditions; surfaced rather than panicking).
pub fn hash_password(password: &str) -> Result<String, String> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| e.to_string())
}

/// Verifies in constant time (Argon2's own comparison). Returns
/// `false` for a malformed stored hash rather than erroring — a
/// corrupt hash should behave identically to a wrong password to
/// the caller, not leak that the row is broken.
#[must_use]
pub fn verify_password(password: &str, stored_hash: &str) -> bool {
    let Ok(parsed) = PasswordHash::new(stored_hash) else {
        return false;
    };
    Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips() {
        let hash = hash_password("correct horse battery staple").unwrap();
        assert!(verify_password("correct horse battery staple", &hash));
        assert!(!verify_password("wrong password", &hash));
    }

    #[test]
    fn malformed_hash_fails_closed() {
        assert!(!verify_password("anything", "not-a-real-hash"));
    }
}
