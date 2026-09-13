//! What a credential has to look like before anything is done with it.
//!
//! Rules rather than a module index, so they live in a file of their
//! own — `mod.rs` declares and re-exports.

use super::{AuthError, MIN_PASSWORD_LEN};

/// # Errors
/// `InvalidEmail` if `email` is empty or has no `@`.
pub fn validate_email(email: &str) -> Result<(), AuthError> {
    if email.trim().is_empty() || !email.contains('@') {
        return Err(AuthError::InvalidEmail);
    }
    Ok(())
}

/// # Errors
/// `PasswordTooShort` if `password` is under [`MIN_PASSWORD_LEN`].
pub fn validate_password(password: &str) -> Result<(), AuthError> {
    if password.len() < MIN_PASSWORD_LEN {
        return Err(AuthError::PasswordTooShort);
    }
    Ok(())
}
