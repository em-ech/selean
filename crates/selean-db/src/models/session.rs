//! Session model for refresh token storage.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A refresh token session for authenticated users.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Session {
    /// Unique session ID.
    pub id: Uuid,
    /// User this session belongs to.
    pub user_id: Uuid,
    /// SHA-256 hash of the refresh token.
    pub token_hash: String,
    /// When this session expires.
    pub expires_at: DateTime<Utc>,
    /// When this session was created.
    pub created_at: DateTime<Utc>,
}
