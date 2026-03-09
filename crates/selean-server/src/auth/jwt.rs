//! JWT token generation and validation.
//!
//! Access tokens are short-lived (15 minutes) for API requests.
//! Refresh tokens are long-lived (7 days) and stored hashed in the database.

use chrono::{Duration, Utc};
use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation, decode, encode};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// JWT configuration.
#[derive(Clone)]
pub struct JwtConfig {
    encoding_key: EncodingKey,
    decoding_key: DecodingKey,
    /// Access token lifetime.
    pub access_token_duration: Duration,
    /// Refresh token lifetime.
    pub refresh_token_duration: Duration,
}

impl JwtConfig {
    /// Creates JWT config from a secret string.
    #[must_use]
    pub fn new(secret: &str) -> Self {
        Self {
            encoding_key: EncodingKey::from_secret(secret.as_bytes()),
            decoding_key: DecodingKey::from_secret(secret.as_bytes()),
            access_token_duration: Duration::minutes(15),
            refresh_token_duration: Duration::days(7),
        }
    }

    /// Creates JWT config from `JWT_SECRET` env var.
    ///
    /// Returns `None` if the env var is not set.
    #[must_use]
    pub fn from_env() -> Option<Self> {
        std::env::var("JWT_SECRET")
            .ok()
            .filter(|s| !s.is_empty())
            .map(|s| Self::new(&s))
    }
}

/// Claims embedded in an access token.
#[derive(Debug, Serialize, Deserialize)]
pub struct AccessClaims {
    /// Subject (user ID).
    pub sub: Uuid,
    /// Expiration time (UNIX timestamp).
    pub exp: i64,
    /// Issued at (UNIX timestamp).
    pub iat: i64,
    /// Token type marker.
    pub typ: String,
}

/// Claims embedded in a refresh token.
#[derive(Debug, Serialize, Deserialize)]
pub struct RefreshClaims {
    /// Subject (user ID).
    pub sub: Uuid,
    /// Expiration time (UNIX timestamp).
    pub exp: i64,
    /// Issued at (UNIX timestamp).
    pub iat: i64,
    /// Token type marker.
    pub typ: String,
    /// Session ID for revocation.
    pub sid: Uuid,
}

/// A pair of access + refresh tokens.
#[derive(Debug, Serialize)]
pub struct TokenPair {
    /// Short-lived access token for API requests.
    pub access_token: String,
    /// Long-lived refresh token for obtaining new access tokens.
    pub refresh_token: String,
    /// Access token expiry in seconds from now.
    pub expires_in: i64,
}

/// Errors from JWT operations.
#[derive(Debug, thiserror::Error)]
pub enum JwtError {
    /// Token encoding failed.
    #[error("failed to encode token: {0}")]
    Encode(#[from] jsonwebtoken::errors::Error),
    /// Token is expired.
    #[error("token expired")]
    Expired,
    /// Token has invalid claims.
    #[error("invalid token: {0}")]
    Invalid(String),
}

/// Generates a new access token for a user.
///
/// # Errors
///
/// Returns an error if token encoding fails.
pub fn create_access_token(config: &JwtConfig, user_id: Uuid) -> Result<String, JwtError> {
    let now = Utc::now();
    let claims = AccessClaims {
        sub: user_id,
        exp: (now + config.access_token_duration).timestamp(),
        iat: now.timestamp(),
        typ: "access".to_string(),
    };
    let token = encode(&Header::default(), &claims, &config.encoding_key)?;
    Ok(token)
}

/// Generates a new refresh token for a user session.
///
/// # Errors
///
/// Returns an error if token encoding fails.
pub fn create_refresh_token(
    config: &JwtConfig,
    user_id: Uuid,
    session_id: Uuid,
) -> Result<String, JwtError> {
    let now = Utc::now();
    let claims = RefreshClaims {
        sub: user_id,
        exp: (now + config.refresh_token_duration).timestamp(),
        iat: now.timestamp(),
        typ: "refresh".to_string(),
        sid: session_id,
    };
    let token = encode(&Header::default(), &claims, &config.encoding_key)?;
    Ok(token)
}

/// Creates a full token pair (access + refresh).
///
/// # Errors
///
/// Returns an error if token encoding fails.
pub fn create_token_pair(
    config: &JwtConfig,
    user_id: Uuid,
    session_id: Uuid,
) -> Result<TokenPair, JwtError> {
    let access_token = create_access_token(config, user_id)?;
    let refresh_token = create_refresh_token(config, user_id, session_id)?;
    Ok(TokenPair {
        access_token,
        refresh_token,
        expires_in: config.access_token_duration.num_seconds(),
    })
}

/// Validates and decodes an access token.
///
/// # Errors
///
/// Returns an error if the token is expired, malformed, or has wrong type.
pub fn validate_access_token(config: &JwtConfig, token: &str) -> Result<AccessClaims, JwtError> {
    let data = decode::<AccessClaims>(token, &config.decoding_key, &Validation::default())
        .map_err(|e| match e.kind() {
            jsonwebtoken::errors::ErrorKind::ExpiredSignature => JwtError::Expired,
            _ => JwtError::Invalid(e.to_string()),
        })?;

    if data.claims.typ != "access" {
        return Err(JwtError::Invalid("not an access token".to_string()));
    }
    Ok(data.claims)
}

/// Validates and decodes a refresh token.
///
/// # Errors
///
/// Returns an error if the token is expired, malformed, or has wrong type.
pub fn validate_refresh_token(config: &JwtConfig, token: &str) -> Result<RefreshClaims, JwtError> {
    let data = decode::<RefreshClaims>(token, &config.decoding_key, &Validation::default())
        .map_err(|e| match e.kind() {
            jsonwebtoken::errors::ErrorKind::ExpiredSignature => JwtError::Expired,
            _ => JwtError::Invalid(e.to_string()),
        })?;

    if data.claims.typ != "refresh" {
        return Err(JwtError::Invalid("not a refresh token".to_string()));
    }
    Ok(data.claims)
}

/// Hashes a refresh token with SHA-256 for database storage.
#[must_use]
pub fn hash_token(token: &str) -> String {
    use sha2::{Digest, Sha256};
    let hash = Sha256::digest(token.as_bytes());
    hex::encode(hash)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn test_config() -> JwtConfig {
        JwtConfig::new("test-jwt-secret-key-for-testing")
    }

    #[test]
    fn access_token_roundtrip() {
        let config = test_config();
        let user_id = Uuid::new_v4();
        let token = create_access_token(&config, user_id).expect("encode");
        let claims = validate_access_token(&config, &token).expect("decode");
        assert_eq!(claims.sub, user_id);
        assert_eq!(claims.typ, "access");
    }

    #[test]
    fn refresh_token_roundtrip() {
        let config = test_config();
        let user_id = Uuid::new_v4();
        let session_id = Uuid::new_v4();
        let token = create_refresh_token(&config, user_id, session_id).expect("encode");
        let claims = validate_refresh_token(&config, &token).expect("decode");
        assert_eq!(claims.sub, user_id);
        assert_eq!(claims.sid, session_id);
        assert_eq!(claims.typ, "refresh");
    }

    #[test]
    fn token_pair_creates_both() {
        let config = test_config();
        let user_id = Uuid::new_v4();
        let session_id = Uuid::new_v4();
        let pair = create_token_pair(&config, user_id, session_id).expect("create pair");
        assert!(!pair.access_token.is_empty());
        assert!(!pair.refresh_token.is_empty());
        assert_eq!(pair.expires_in, 900); // 15 minutes
    }

    #[test]
    fn wrong_type_rejected() {
        let config = test_config();
        let user_id = Uuid::new_v4();
        let session_id = Uuid::new_v4();

        let access = create_access_token(&config, user_id).expect("encode");
        let refresh = create_refresh_token(&config, user_id, session_id).expect("encode");

        // Access token rejected as refresh.
        assert!(validate_refresh_token(&config, &access).is_err());
        // Refresh token rejected as access.
        assert!(validate_access_token(&config, &refresh).is_err());
    }

    #[test]
    fn wrong_secret_rejected() {
        let config = test_config();
        let other_config = JwtConfig::new("different-secret");
        let user_id = Uuid::new_v4();

        let token = create_access_token(&config, user_id).expect("encode");
        assert!(validate_access_token(&other_config, &token).is_err());
    }

    #[test]
    fn expired_token_rejected() {
        let mut config = test_config();
        config.access_token_duration = Duration::seconds(-120);
        let user_id = Uuid::new_v4();
        let token = create_access_token(&config, user_id).expect("encode");
        let err = validate_access_token(&test_config(), &token).unwrap_err();
        assert!(matches!(err, JwtError::Expired));
    }

    #[test]
    fn hash_token_is_deterministic() {
        let h1 = hash_token("my-refresh-token");
        let h2 = hash_token("my-refresh-token");
        assert_eq!(h1, h2);
        assert_ne!(h1, hash_token("different-token"));
    }

    #[test]
    fn hash_token_is_hex() {
        let h = hash_token("test");
        assert!(h.chars().all(|c| c.is_ascii_hexdigit()));
        assert_eq!(h.len(), 64); // SHA-256 = 32 bytes = 64 hex chars
    }
}
