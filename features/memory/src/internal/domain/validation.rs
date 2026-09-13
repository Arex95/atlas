//! What a memory key has to look like.
//!
//! A rule rather than a module index, so it lives in a file of its
//! own — `mod.rs` declares and re-exports.

use super::MemoryError;

/// # Errors
/// `EmptyKey` if `key` is blank once trimmed — a keyless memory is
/// unreachable by every read path, so writing one is always a bug
/// rather than an unusual choice.
pub fn validate_key(key: &str) -> Result<(), MemoryError> {
    if key.trim().is_empty() {
        return Err(MemoryError::EmptyKey);
    }
    Ok(())
}
