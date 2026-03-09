//! User model.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A registered user.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct User {
    /// Unique user ID.
    pub id: Uuid,
    /// Email address (unique).
    pub email: String,
    /// Display name shown in the UI.
    pub display_name: String,
    /// Argon2 password hash. `None` for OAuth-only accounts.
    #[serde(skip_serializing)]
    pub password_hash: Option<String>,
    /// Avatar image URL.
    pub avatar_url: Option<String>,
    /// When the user was created.
    pub created_at: DateTime<Utc>,
    /// When the user was last updated.
    pub updated_at: DateTime<Utc>,
}

/// Data required to create a new user.
#[derive(Debug, Clone)]
pub struct CreateUser {
    /// Email address.
    pub email: String,
    /// Display name.
    pub display_name: String,
    /// Password hash (None for OAuth signup).
    pub password_hash: Option<String>,
    /// Avatar URL (None by default).
    pub avatar_url: Option<String>,
}
