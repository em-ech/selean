//! Session (refresh token) queries.

use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::DbError;
use crate::models::session::Session;

/// Creates a new session with a hashed refresh token.
///
/// # Errors
///
/// Returns an error if the query fails.
pub async fn create_session(
    pool: &PgPool,
    user_id: Uuid,
    token_hash: &str,
    expires_at: DateTime<Utc>,
) -> Result<Session, DbError> {
    let id = Uuid::new_v4();
    let session = sqlx::query_as::<_, Session>(
        r"
        INSERT INTO sessions (id, user_id, token_hash, expires_at)
        VALUES ($1, $2, $3, $4)
        RETURNING *
        ",
    )
    .bind(id)
    .bind(user_id)
    .bind(token_hash)
    .bind(expires_at)
    .fetch_one(pool)
    .await?;

    Ok(session)
}

/// Finds a session by token hash (for refresh token validation).
///
/// Only returns non-expired sessions.
///
/// # Errors
///
/// Returns `NotFound` if no valid session exists for this token.
pub async fn get_session_by_token(pool: &PgPool, token_hash: &str) -> Result<Session, DbError> {
    sqlx::query_as::<_, Session>(
        "SELECT * FROM sessions WHERE token_hash = $1 AND expires_at > now()",
    )
    .bind(token_hash)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| DbError::NotFound("session".to_string()))
}

/// Deletes a session (logout).
///
/// # Errors
///
/// Returns an error if the query fails.
pub async fn delete_session(pool: &PgPool, id: Uuid) -> Result<(), DbError> {
    sqlx::query("DELETE FROM sessions WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

/// Deletes all sessions for a user (logout everywhere).
///
/// # Errors
///
/// Returns an error if the query fails.
pub async fn delete_user_sessions(pool: &PgPool, user_id: Uuid) -> Result<u64, DbError> {
    let result = sqlx::query("DELETE FROM sessions WHERE user_id = $1")
        .bind(user_id)
        .execute(pool)
        .await?;

    Ok(result.rows_affected())
}

/// Deletes all expired sessions (cleanup job).
///
/// # Errors
///
/// Returns an error if the query fails.
pub async fn cleanup_expired(pool: &PgPool) -> Result<u64, DbError> {
    let result = sqlx::query("DELETE FROM sessions WHERE expires_at <= now()")
        .execute(pool)
        .await?;

    Ok(result.rows_affected())
}
