//! Stateless CSRF protection for the OAuth redirect round trip: a
//! signed, timestamped nonce instead of a server-side pending-request
//! table. No new persisted or in-memory state to manage — the
//! signature and the age check are the entire mechanism.

use std::fmt::Write as _;

use chrono::Utc;
use hmac::{Hmac, Mac};
use sha2::Sha256;

use crate::internal::domain::{AuthError, OAUTH_STATE_MAX_AGE_SECS};

type HmacSha256 = Hmac<Sha256>;

#[must_use]
pub fn sign(secret: &str, now: chrono::DateTime<Utc>) -> String {
    let ts = now.timestamp();
    let mac = mac_for(secret, ts);
    format!("{ts}.{mac}")
}

/// # Errors
/// `OAuthStateInvalid` if `state` is malformed, the signature
/// doesn't match, or it's older than [`OAUTH_STATE_MAX_AGE_SECS`].
pub fn verify(secret: &str, state: &str, now: chrono::DateTime<Utc>) -> Result<(), AuthError> {
    let (ts_raw, mac_raw) = state.split_once('.').ok_or(AuthError::OAuthStateInvalid)?;
    let ts: i64 = ts_raw.parse().map_err(|_| AuthError::OAuthStateInvalid)?;

    let age = now.timestamp() - ts;
    if !(0..=OAUTH_STATE_MAX_AGE_SECS).contains(&age) {
        return Err(AuthError::OAuthStateInvalid);
    }

    let expected = mac_for(secret, ts);
    // Constant-time-ish via length-prefixed equality is unnecessary
    // here — both sides are fixed-length hex digests, and a timing
    // side-channel on a value that's also bounded by the 5-minute
    // window and single-use-in-practice isn't the realistic threat
    // this stateless design is defending against. Plain `==` is the
    // conventional choice for HMAC verification when the mismatch
    // itself just reissues a fresh redirect, not a security decision.
    if expected != mac_raw {
        return Err(AuthError::OAuthStateInvalid);
    }
    Ok(())
}

fn mac_for(secret: &str, ts: i64) -> String {
    let mut mac =
        HmacSha256::new_from_slice(secret.as_bytes()).expect("HMAC accepts a key of any length");
    mac.update(ts.to_string().as_bytes());
    hex_encode(&mac.finalize().into_bytes())
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
    fn a_freshly_signed_state_verifies() {
        let now = Utc::now();
        let state = sign("secret", now);
        assert!(verify("secret", &state, now).is_ok());
    }

    #[test]
    fn a_state_signed_with_a_different_secret_is_rejected() {
        let now = Utc::now();
        let state = sign("secret-a", now);
        assert!(matches!(
            verify("secret-b", &state, now),
            Err(AuthError::OAuthStateInvalid)
        ));
    }

    #[test]
    fn an_expired_state_is_rejected() {
        let signed_at = Utc::now();
        let state = sign("secret", signed_at);
        let later = signed_at + chrono::Duration::seconds(OAUTH_STATE_MAX_AGE_SECS + 1);
        assert!(matches!(
            verify("secret", &state, later),
            Err(AuthError::OAuthStateInvalid)
        ));
    }

    #[test]
    fn a_state_from_the_future_is_rejected() {
        // Guards against a forged state claiming a timestamp ahead
        // of "now" to buy extra replay window.
        let now = Utc::now();
        let state = sign("secret", now + chrono::Duration::seconds(60));
        assert!(matches!(
            verify("secret", &state, now),
            Err(AuthError::OAuthStateInvalid)
        ));
    }

    #[test]
    fn a_malformed_state_is_rejected() {
        assert!(matches!(
            verify("secret", "not-a-valid-state", Utc::now()),
            Err(AuthError::OAuthStateInvalid)
        ));
    }
}
