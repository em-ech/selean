//! User CRUD queries.

use sqlx::PgPool;
use uuid::Uuid;

use crate::DbError;
use crate::models::user::{CreateUser, User};

/// Creates a new user and returns the full row.
///
/// # Errors
///
/// Returns an error if the email already exists or the query fails.
pub async fn create_user(pool: &PgPool, input: &CreateUser) -> Result<User, DbError> {
    let id = Uuid::new_v4();
    let user = sqlx::query_as::<_, User>(
        r"
        INSERT INTO users (id, email, display_name, password_hash, avatar_url)
        VALUES ($1, $2, $3, $4, $5)
        RETURNING *
        ",
    )
    .bind(id)
    .bind(&input.email)
    .bind(&input.display_name)
    .bind(&input.password_hash)
    .bind(&input.avatar_url)
    .fetch_one(pool)
    .await?;

    Ok(user)
}

/// Finds a user by ID.
///
/// # Errors
///
/// Returns `NotFound` if the user does not exist.
pub async fn get_user(pool: &PgPool, id: Uuid) -> Result<User, DbError> {
    sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = $1")
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| DbError::NotFound("user".to_string()))
}

/// Finds a user by email address.
///
/// # Errors
///
/// Returns `NotFound` if no user with that email exists.
pub async fn get_user_by_email(pool: &PgPool, email: &str) -> Result<User, DbError> {
    sqlx::query_as::<_, User>("SELECT * FROM users WHERE email = $1")
        .bind(email)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| DbError::NotFound("user".to_string()))
}

/// Finds a user by email, returning `None` if not found (no error).
///
/// # Errors
///
/// Returns an error only on query failure.
pub async fn find_user_by_email(pool: &PgPool, email: &str) -> Result<Option<User>, DbError> {
    let user = sqlx::query_as::<_, User>("SELECT * FROM users WHERE email = $1")
        .bind(email)
        .fetch_optional(pool)
        .await?;

    Ok(user)
}

/// Updates a user's display name and avatar URL.
///
/// # Errors
///
/// Returns an error if the user does not exist or the query fails.
pub async fn update_user_profile(
    pool: &PgPool,
    id: Uuid,
    display_name: &str,
    avatar_url: Option<&str>,
) -> Result<User, DbError> {
    sqlx::query_as::<_, User>(
        r"
        UPDATE users
        SET display_name = $2, avatar_url = $3, updated_at = now()
        WHERE id = $1
        RETURNING *
        ",
    )
    .bind(id)
    .bind(display_name)
    .bind(avatar_url)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| DbError::NotFound("user".to_string()))
}

/// Updates a user's password hash.
///
/// # Errors
///
/// Returns an error if the user does not exist or the query fails.
pub async fn update_user_password(
    pool: &PgPool,
    id: Uuid,
    password_hash: &str,
) -> Result<(), DbError> {
    let result =
        sqlx::query("UPDATE users SET password_hash = $2, updated_at = now() WHERE id = $1")
            .bind(id)
            .bind(password_hash)
            .execute(pool)
            .await?;

    if result.rows_affected() == 0 {
        return Err(DbError::NotFound("user".to_string()));
    }
    Ok(())
}
