//! Document and version queries.

use sqlx::PgPool;
use uuid::Uuid;

use crate::DbError;
use crate::models::document::{Document, DocumentVersion};

/// Creates a new document in a workspace.
///
/// # Errors
///
/// Returns an error if the query fails.
pub async fn create_document(
    pool: &PgPool,
    workspace_id: Uuid,
    name: &str,
    created_by: Uuid,
) -> Result<Document, DbError> {
    let id = Uuid::new_v4();
    let doc = sqlx::query_as::<_, Document>(
        r"
        INSERT INTO documents (id, workspace_id, name, created_by)
        VALUES ($1, $2, $3, $4)
        RETURNING *
        ",
    )
    .bind(id)
    .bind(workspace_id)
    .bind(name)
    .bind(created_by)
    .fetch_one(pool)
    .await?;

    Ok(doc)
}

/// Gets a document by ID (excludes soft-deleted).
///
/// # Errors
///
/// Returns `NotFound` if the document does not exist or is deleted.
pub async fn get_document(pool: &PgPool, id: Uuid) -> Result<Document, DbError> {
    sqlx::query_as::<_, Document>("SELECT * FROM documents WHERE id = $1 AND deleted_at IS NULL")
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| DbError::NotFound("document".to_string()))
}

/// Lists active documents in a workspace (paginated, excludes deleted).
///
/// # Errors
///
/// Returns an error if the query fails.
pub async fn list_documents(
    pool: &PgPool,
    workspace_id: Uuid,
    limit: i64,
    offset: i64,
) -> Result<Vec<Document>, DbError> {
    let docs = sqlx::query_as::<_, Document>(
        r"
        SELECT * FROM documents
        WHERE workspace_id = $1 AND deleted_at IS NULL
        ORDER BY updated_at DESC
        LIMIT $2 OFFSET $3
        ",
    )
    .bind(workspace_id)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await?;

    Ok(docs)
}

/// Renames a document.
///
/// # Errors
///
/// Returns `NotFound` if the document does not exist.
pub async fn rename_document(pool: &PgPool, id: Uuid, name: &str) -> Result<Document, DbError> {
    sqlx::query_as::<_, Document>(
        r"
        UPDATE documents SET name = $2, updated_at = now()
        WHERE id = $1 AND deleted_at IS NULL
        RETURNING *
        ",
    )
    .bind(id)
    .bind(name)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| DbError::NotFound("document".to_string()))
}

/// Soft-deletes a document.
///
/// # Errors
///
/// Returns `NotFound` if the document does not exist.
pub async fn soft_delete_document(pool: &PgPool, id: Uuid) -> Result<(), DbError> {
    let result = sqlx::query(
        "UPDATE documents SET deleted_at = now(), updated_at = now() WHERE id = $1 AND deleted_at IS NULL",
    )
    .bind(id)
    .execute(pool)
    .await?;

    if result.rows_affected() == 0 {
        return Err(DbError::NotFound("document".to_string()));
    }
    Ok(())
}

/// Hard-deletes a document and all its versions.
///
/// # Errors
///
/// Returns `NotFound` if the document does not exist.
pub async fn hard_delete_document(pool: &PgPool, id: Uuid) -> Result<(), DbError> {
    let result = sqlx::query("DELETE FROM documents WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;

    if result.rows_affected() == 0 {
        return Err(DbError::NotFound("document".to_string()));
    }
    Ok(())
}

/// Saves a new document version. Auto-increments the version number.
///
/// # Errors
///
/// Returns an error if the document does not exist or the query fails.
pub async fn save_version(
    pool: &PgPool,
    document_id: Uuid,
    data: &serde_json::Value,
    created_by: Uuid,
) -> Result<DocumentVersion, DbError> {
    let id = Uuid::new_v4();

    let version = sqlx::query_as::<_, DocumentVersion>(
        r"
        INSERT INTO document_versions (id, document_id, version, data, created_by)
        VALUES (
            $1, $2,
            COALESCE((SELECT MAX(version) FROM document_versions WHERE document_id = $2), 0) + 1,
            $3, $4
        )
        RETURNING *
        ",
    )
    .bind(id)
    .bind(document_id)
    .bind(data)
    .bind(created_by)
    .fetch_one(pool)
    .await?;

    // Touch the document's updated_at.
    sqlx::query("UPDATE documents SET updated_at = now() WHERE id = $1")
        .bind(document_id)
        .execute(pool)
        .await?;

    Ok(version)
}

/// Gets the latest version of a document.
///
/// # Errors
///
/// Returns `NotFound` if no versions exist.
pub async fn get_latest_version(
    pool: &PgPool,
    document_id: Uuid,
) -> Result<DocumentVersion, DbError> {
    sqlx::query_as::<_, DocumentVersion>(
        r"
        SELECT * FROM document_versions
        WHERE document_id = $1
        ORDER BY version DESC
        LIMIT 1
        ",
    )
    .bind(document_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| DbError::NotFound("document version".to_string()))
}

/// Gets a specific version of a document.
///
/// # Errors
///
/// Returns `NotFound` if the version does not exist.
pub async fn get_version(
    pool: &PgPool,
    document_id: Uuid,
    version: i32,
) -> Result<DocumentVersion, DbError> {
    sqlx::query_as::<_, DocumentVersion>(
        "SELECT * FROM document_versions WHERE document_id = $1 AND version = $2",
    )
    .bind(document_id)
    .bind(version)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| DbError::NotFound("document version".to_string()))
}

/// Lists version history for a document (most recent first).
///
/// # Errors
///
/// Returns an error if the query fails.
pub async fn list_versions(
    pool: &PgPool,
    document_id: Uuid,
    limit: i64,
) -> Result<Vec<DocumentVersion>, DbError> {
    let versions = sqlx::query_as::<_, DocumentVersion>(
        r"
        SELECT * FROM document_versions
        WHERE document_id = $1
        ORDER BY version DESC
        LIMIT $2
        ",
    )
    .bind(document_id)
    .bind(limit)
    .fetch_all(pool)
    .await?;

    Ok(versions)
}
