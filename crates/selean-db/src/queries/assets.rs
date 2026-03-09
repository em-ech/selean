//! Asset queries.

use sqlx::PgPool;
use uuid::Uuid;

use crate::DbError;
use crate::models::asset::Asset;

/// Records a new asset upload.
///
/// # Errors
///
/// Returns an error if the query fails.
pub async fn create_asset(
    pool: &PgPool,
    workspace_id: Uuid,
    filename: &str,
    content_type: &str,
    size_bytes: i64,
    storage_key: &str,
    uploaded_by: Uuid,
) -> Result<Asset, DbError> {
    let id = Uuid::new_v4();
    let asset = sqlx::query_as::<_, Asset>(
        r"
        INSERT INTO assets (id, workspace_id, filename, content_type, size_bytes, storage_key, uploaded_by)
        VALUES ($1, $2, $3, $4, $5, $6, $7)
        RETURNING *
        ",
    )
    .bind(id)
    .bind(workspace_id)
    .bind(filename)
    .bind(content_type)
    .bind(size_bytes)
    .bind(storage_key)
    .bind(uploaded_by)
    .fetch_one(pool)
    .await?;

    Ok(asset)
}

/// Gets an asset by ID.
///
/// # Errors
///
/// Returns `NotFound` if the asset does not exist.
pub async fn get_asset(pool: &PgPool, id: Uuid) -> Result<Asset, DbError> {
    sqlx::query_as::<_, Asset>("SELECT * FROM assets WHERE id = $1")
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| DbError::NotFound("asset".to_string()))
}

/// Lists assets in a workspace.
///
/// # Errors
///
/// Returns an error if the query fails.
pub async fn list_assets(
    pool: &PgPool,
    workspace_id: Uuid,
    limit: i64,
    offset: i64,
) -> Result<Vec<Asset>, DbError> {
    let assets = sqlx::query_as::<_, Asset>(
        r"
        SELECT * FROM assets
        WHERE workspace_id = $1
        ORDER BY created_at DESC
        LIMIT $2 OFFSET $3
        ",
    )
    .bind(workspace_id)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await?;

    Ok(assets)
}

/// Deletes an asset record. Does NOT delete from storage backend.
///
/// # Errors
///
/// Returns `NotFound` if the asset does not exist.
pub async fn delete_asset(pool: &PgPool, id: Uuid) -> Result<String, DbError> {
    let row: Option<(String,)> =
        sqlx::query_as("DELETE FROM assets WHERE id = $1 RETURNING storage_key")
            .bind(id)
            .fetch_optional(pool)
            .await?;

    row.map(|(key,)| key)
        .ok_or_else(|| DbError::NotFound("asset".to_string()))
}

/// Gets total storage usage for a workspace in bytes.
///
/// # Errors
///
/// Returns an error if the query fails.
pub async fn workspace_storage_bytes(pool: &PgPool, workspace_id: Uuid) -> Result<i64, DbError> {
    let row: (Option<i64>,) =
        sqlx::query_as("SELECT COALESCE(SUM(size_bytes), 0) FROM assets WHERE workspace_id = $1")
            .bind(workspace_id)
            .fetch_one(pool)
            .await?;

    Ok(row.0.unwrap_or(0))
}
