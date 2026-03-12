//! Document service layer.
//!
//! Business logic for document CRUD and version management, extracted
//! from route handlers. Each method takes a `PgPool` and parameters,
//! performs authorization and billing checks, and returns domain models
//! or `ServiceError`.

use sqlx::PgPool;
use uuid::Uuid;

use selean_db::models::document::{Document, DocumentVersion};
use selean_db::models::workspace::WorkspaceRole;

use super::billing::check_document_limit;
use super::errors::ServiceError;
use crate::rbac;

/// Creates a new document in a workspace.
///
/// Requires `Editor`+ role. Checks billing tier document limit.
///
/// # Errors
///
/// Returns `ServiceError` on permission, billing, or database failures.
pub async fn create_document(
    pool: &PgPool,
    workspace_id: Uuid,
    name: &str,
    user_id: Uuid,
) -> Result<Document, ServiceError> {
    rbac::require_role(pool, workspace_id, user_id, WorkspaceRole::Editor)
        .await
        .map_err(|_| ServiceError::forbidden("insufficient permissions"))?;

    check_document_limit(pool, workspace_id).await?;

    let doc =
        selean_db::queries::documents::create_document(pool, workspace_id, name, user_id).await?;

    Ok(doc)
}

/// Gets a document by ID. Requires `Viewer`+ role in the document's workspace.
///
/// # Errors
///
/// Returns `ServiceError` on permission or database failures.
pub async fn get_document(
    pool: &PgPool,
    document_id: Uuid,
    user_id: Uuid,
) -> Result<Document, ServiceError> {
    let doc = selean_db::queries::documents::get_document(pool, document_id).await?;

    rbac::require_role(pool, doc.workspace_id, user_id, WorkspaceRole::Viewer)
        .await
        .map_err(|_| ServiceError::forbidden("insufficient permissions"))?;

    Ok(doc)
}

/// Lists documents in a workspace. Requires `Viewer`+ role.
///
/// # Errors
///
/// Returns `ServiceError` on permission or database failures.
pub async fn list_documents(
    pool: &PgPool,
    workspace_id: Uuid,
    user_id: Uuid,
    limit: i64,
    offset: i64,
) -> Result<Vec<Document>, ServiceError> {
    rbac::require_role(pool, workspace_id, user_id, WorkspaceRole::Viewer)
        .await
        .map_err(|_| ServiceError::forbidden("insufficient permissions"))?;

    let limit = limit.clamp(1, 100);
    let offset = offset.max(0);

    let docs =
        selean_db::queries::documents::list_documents(pool, workspace_id, limit, offset).await?;

    Ok(docs)
}

/// Saves a new version of a document. Requires `Editor`+ role in the
/// document's workspace.
///
/// # Errors
///
/// Returns `ServiceError` on permission or database failures.
pub async fn save_version(
    pool: &PgPool,
    document_id: Uuid,
    data: &serde_json::Value,
    user_id: Uuid,
) -> Result<DocumentVersion, ServiceError> {
    let doc = selean_db::queries::documents::get_document(pool, document_id).await?;

    rbac::require_role(pool, doc.workspace_id, user_id, WorkspaceRole::Editor)
        .await
        .map_err(|_| ServiceError::forbidden("insufficient permissions"))?;

    let version =
        selean_db::queries::documents::save_version(pool, document_id, data, user_id).await?;

    Ok(version)
}

/// Gets the latest version of a document. Requires `Viewer`+ role.
///
/// # Errors
///
/// Returns `ServiceError` on permission or database failures.
pub async fn get_latest_version(
    pool: &PgPool,
    document_id: Uuid,
    user_id: Uuid,
) -> Result<DocumentVersion, ServiceError> {
    let doc = selean_db::queries::documents::get_document(pool, document_id).await?;

    rbac::require_role(pool, doc.workspace_id, user_id, WorkspaceRole::Viewer)
        .await
        .map_err(|_| ServiceError::forbidden("insufficient permissions"))?;

    let version = selean_db::queries::documents::get_latest_version(pool, document_id).await?;

    Ok(version)
}

/// Lists version history for a document. Requires `Viewer`+ role.
///
/// # Errors
///
/// Returns `ServiceError` on permission or database failures.
pub async fn list_versions(
    pool: &PgPool,
    document_id: Uuid,
    user_id: Uuid,
    limit: i64,
) -> Result<Vec<DocumentVersion>, ServiceError> {
    let doc = selean_db::queries::documents::get_document(pool, document_id).await?;

    rbac::require_role(pool, doc.workspace_id, user_id, WorkspaceRole::Viewer)
        .await
        .map_err(|_| ServiceError::forbidden("insufficient permissions"))?;

    let limit = limit.clamp(1, 100);
    let versions = selean_db::queries::documents::list_versions(pool, document_id, limit).await?;

    Ok(versions)
}
