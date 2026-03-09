//! Password hashing and verification using Argon2.

use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier, password_hash::SaltString};

/// Errors from password operations.
#[derive(Debug, thiserror::Error)]
pub enum PasswordError {
    /// Hashing failed.
    #[error("password hashing failed: {0}")]
    Hash(String),
    /// Verification failed (wrong password).
    #[error("invalid password")]
    Invalid,
}

/// Hashes a password using Argon2id with a random salt.
///
/// # Errors
///
/// Returns an error if hashing fails.
pub fn hash_password(password: &str) -> Result<String, PasswordError> {
    let salt = SaltString::generate(&mut rand::thread_rng());
    let argon2 = Argon2::default();
    let hash = argon2
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| PasswordError::Hash(e.to_string()))?;
    Ok(hash.to_string())
}

/// Verifies a password against a stored Argon2 hash.
///
/// # Errors
///
/// Returns `PasswordError::Invalid` if the password does not match.
pub fn verify_password(password: &str, hash: &str) -> Result<(), PasswordError> {
    let parsed = PasswordHash::new(hash).map_err(|e| PasswordError::Hash(e.to_string()))?;
    Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .map_err(|_| PasswordError::Invalid)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn hash_and_verify() {
        let hash = hash_password("my-password").expect("hash");
        assert!(hash.starts_with("$argon2"));
        verify_password("my-password", &hash).expect("verify");
    }

    #[test]
    fn wrong_password_fails() {
        let hash = hash_password("correct").expect("hash");
        let err = verify_password("wrong", &hash).unwrap_err();
        assert!(matches!(err, PasswordError::Invalid));
    }

    #[test]
    fn different_hashes_for_same_password() {
        let h1 = hash_password("same").expect("hash1");
        let h2 = hash_password("same").expect("hash2");
        assert_ne!(h1, h2, "random salts should produce different hashes");
        // But both should verify.
        verify_password("same", &h1).expect("verify1");
        verify_password("same", &h2).expect("verify2");
    }

    #[test]
    fn empty_password_works() {
        let hash = hash_password("").expect("hash");
        verify_password("", &hash).expect("verify");
        assert!(verify_password("notempty", &hash).is_err());
    }
}
