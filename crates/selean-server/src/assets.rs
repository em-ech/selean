//! Asset upload, serving, listing, and deletion route handlers.
//!
//! Assets are stored in a pluggable storage backend (local filesystem or S3)
//! and tracked in the database. Upload enforces content-type allowlists,
//! file size limits, and workspace storage quotas.

use axum::{
    Json,
    extract::{FromRequest, Multipart, Path, Query, State},
    http::{StatusCode, header},
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::rbac::{extract_current_user, require_role, resolve_billing_tier};
use crate::state::AppState;
use selean_db::models::workspace::WorkspaceRole;

/// Allowed MIME types for asset uploads.
const ALLOWED_CONTENT_TYPES: &[&str] = &[
    "image/png",
    "image/jpeg",
    "image/webp",
    "image/svg+xml",
    "font/ttf",
    "font/otf",
    "font/woff",
    "font/woff2",
    "application/pdf",
];

/// Maximum asset file size: 10 MiB.
const MAX_ASSET_BYTES: usize = 10 * 1024 * 1024;

/// JSON response for a single asset.
#[derive(Debug, Serialize)]
struct AssetResponse {
    id: Uuid,
    workspace_id: Uuid,
    filename: String,
    content_type: String,
    size_bytes: i64,
    created_at: String,
    url: String,
}

impl AssetResponse {
    fn from_asset(asset: &selean_db::models::asset::Asset) -> Self {
        Self {
            id: asset.id,
            workspace_id: asset.workspace_id,
            filename: asset.filename.clone(),
            content_type: asset.content_type.clone(),
            size_bytes: asset.size_bytes,
            created_at: asset.created_at.to_rfc3339(),
            url: format!("/api/assets/{}", asset.id),
        }
    }
}

/// Query parameters for listing assets.
#[derive(Debug, Deserialize)]
pub struct ListAssetsQuery {
    /// Maximum number of assets to return (default 50, max 200).
    limit: Option<i64>,
    /// Offset for pagination (default 0).
    offset: Option<i64>,
}

/// Sanitizes a filename by stripping path separators, replacing `..` with `_`,
/// and truncating to 255 characters.
fn sanitize_filename(name: &str) -> String {
    let mut cleaned = String::with_capacity(name.len());
    for ch in name.chars() {
        if ch == '/' || ch == '\\' {
            continue;
        }
        cleaned.push(ch);
    }
    cleaned = cleaned.replace("..", "_");
    cleaned.truncate(255);
    cleaned
}

/// Returns `true` if the content type is in the allowlist.
fn is_allowed_content_type(ct: &str) -> bool {
    ALLOWED_CONTENT_TYPES.contains(&ct)
}

/// Returns a JSON error response.
fn error_response(status: StatusCode, message: &str) -> axum::response::Response {
    (status, Json(serde_json::json!({ "error": message }))).into_response()
}

/// Handles asset upload via multipart form.
///
/// Expects a `file` field (the binary data) and a `workspace_id` field.
/// Validates content type, file size, RBAC (Editor role), and storage quota.
#[allow(clippy::result_large_err)]
pub async fn upload_asset(
    State(state): State<AppState>,
    request: axum::http::Request<axum::body::Body>,
) -> axum::response::Response {
    let Ok(user) = extract_current_user(request.extensions()) else {
        return error_response(StatusCode::UNAUTHORIZED, "not authenticated");
    };

    let Ok(pool) = state.require_db() else {
        return error_response(StatusCode::SERVICE_UNAVAILABLE, "database not configured");
    };

    let Ok(mut multipart) = axum::extract::Multipart::from_request(request, &state).await else {
        return error_response(StatusCode::BAD_REQUEST, "invalid multipart request");
    };

    let (file_data, filename, content_type, workspace_id) =
        match read_upload_fields(&mut multipart).await {
            Ok(v) => v,
            Err(resp) => return resp,
        };

    if !is_allowed_content_type(&content_type) {
        return error_response(
            StatusCode::BAD_REQUEST,
            &format!("content type '{content_type}' is not allowed"),
        );
    }

    if file_data.is_empty() {
        return error_response(StatusCode::BAD_REQUEST, "file is empty");
    }

    if file_data.len() > MAX_ASSET_BYTES {
        return error_response(
            StatusCode::PAYLOAD_TOO_LARGE,
            &format!(
                "file size {} exceeds maximum {}",
                file_data.len(),
                MAX_ASSET_BYTES
            ),
        );
    }

    // RBAC: require Editor role.
    if let Err(resp) = require_role(pool, workspace_id, user.id, WorkspaceRole::Editor).await {
        return resp;
    }

    // Check storage quota.
    if let Err(resp) = check_storage_quota(pool, workspace_id, file_data.len()).await {
        return resp;
    }

    let safe_filename = sanitize_filename(&filename);
    let asset_id = Uuid::new_v4();
    let storage_key = format!("{workspace_id}/{asset_id}/{safe_filename}");

    // Write to storage backend.
    if let Err(e) = state
        .storage
        .put(&storage_key, file_data.clone(), &content_type)
        .await
    {
        return error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("storage write failed: {e}"),
        );
    }

    // Record in database.
    let size_bytes = i64::try_from(file_data.len()).unwrap_or(i64::MAX);
    match selean_db::queries::assets::create_asset(
        pool,
        workspace_id,
        &safe_filename,
        &content_type,
        size_bytes,
        &storage_key,
        user.id,
    )
    .await
    {
        Ok(asset) => (StatusCode::CREATED, Json(AssetResponse::from_asset(&asset))).into_response(),
        Err(e) => {
            // Attempt to clean up the storage object on DB failure.
            if let Err(cleanup_err) = state.storage.delete(&storage_key).await {
                tracing::error!(
                    "failed to clean up storage key {storage_key} after DB error: {cleanup_err}"
                );
            }
            error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                &format!("database error: {e}"),
            )
        }
    }
}

/// Checks whether the workspace has enough storage quota for the upload.
#[allow(clippy::result_large_err)]
async fn check_storage_quota(
    pool: &sqlx::PgPool,
    workspace_id: Uuid,
    file_size: usize,
) -> Result<(), axum::response::Response> {
    let workspace = selean_db::queries::workspaces::get_workspace(pool, workspace_id)
        .await
        .map_err(|e| {
            error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                &format!("workspace lookup failed: {e}"),
            )
        })?;

    let tier = resolve_billing_tier(&workspace.billing_tier)?;

    if let Some(max_bytes) = tier.max_storage_bytes() {
        let current_bytes = selean_db::queries::assets::workspace_storage_bytes(pool, workspace_id)
            .await
            .map_err(|e| {
                error_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    &format!("storage usage query failed: {e}"),
                )
            })?;

        let current = u64::try_from(current_bytes).unwrap_or(0);
        let total = current + file_size as u64;
        if total > max_bytes {
            return Err(error_response(
                StatusCode::PAYLOAD_TOO_LARGE,
                &format!(
                    "workspace storage quota exceeded (used: {current_bytes}, limit: {max_bytes})"
                ),
            ));
        }
    }

    Ok(())
}

/// Reads the `file` and `workspace_id` fields from the multipart upload.
///
/// Returns `(file_bytes, filename, content_type, workspace_id)`.
#[allow(clippy::result_large_err)]
async fn read_upload_fields(
    multipart: &mut Multipart,
) -> Result<(Vec<u8>, String, String, Uuid), axum::response::Response> {
    let mut file_data: Option<Vec<u8>> = None;
    let mut filename: Option<String> = None;
    let mut content_type: Option<String> = None;
    let mut workspace_id: Option<Uuid> = None;

    while let Ok(Some(field)) = multipart.next_field().await {
        let field_name = field.name().map(str::to_string);
        match field_name.as_deref() {
            Some("file") => {
                filename = field.file_name().map(str::to_string);
                content_type = field.content_type().map(str::to_string);
                let bytes = field.bytes().await.map_err(|e| {
                    error_response(
                        StatusCode::BAD_REQUEST,
                        &format!("failed to read file field: {e}"),
                    )
                })?;
                file_data = Some(bytes.to_vec());
            }
            Some("workspace_id") => {
                let text = field.text().await.map_err(|e| {
                    error_response(
                        StatusCode::BAD_REQUEST,
                        &format!("failed to read workspace_id field: {e}"),
                    )
                })?;
                workspace_id = Some(Uuid::parse_str(&text).map_err(|_| {
                    error_response(StatusCode::BAD_REQUEST, "invalid workspace_id UUID")
                })?);
            }
            _ => {}
        }
    }

    let file_data =
        file_data.ok_or_else(|| error_response(StatusCode::BAD_REQUEST, "missing 'file' field"))?;
    let filename = filename.unwrap_or_else(|| "unnamed".to_string());
    let content_type = content_type.unwrap_or_else(|| "application/octet-stream".to_string());
    let workspace_id = workspace_id
        .ok_or_else(|| error_response(StatusCode::BAD_REQUEST, "missing 'workspace_id' field"))?;

    Ok((file_data, filename, content_type, workspace_id))
}

/// Serves an asset file by ID with appropriate content headers.
///
/// Requires `Viewer` role in the asset's workspace.
pub async fn serve_asset(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    request: axum::http::Request<axum::body::Body>,
) -> axum::response::Response {
    let Ok(user) = extract_current_user(request.extensions()) else {
        return error_response(StatusCode::UNAUTHORIZED, "not authenticated");
    };

    let Ok(pool) = state.require_db() else {
        return error_response(StatusCode::SERVICE_UNAVAILABLE, "database not configured");
    };

    let asset = match selean_db::queries::assets::get_asset(pool, id).await {
        Ok(a) => a,
        Err(selean_db::DbError::NotFound(_)) => {
            return error_response(StatusCode::NOT_FOUND, "asset not found");
        }
        Err(e) => {
            return error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                &format!("database error: {e}"),
            );
        }
    };

    if let Err(resp) = require_role(pool, asset.workspace_id, user.id, WorkspaceRole::Viewer).await
    {
        return resp;
    }

    match state.storage.get(&asset.storage_key).await {
        Ok((data, _)) => {
            let ct = asset.content_type.clone();
            (
                StatusCode::OK,
                [
                    (header::CONTENT_TYPE, ct),
                    (header::CONTENT_DISPOSITION, "inline".to_string()),
                ],
                data,
            )
                .into_response()
        }
        Err(e) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("storage read failed: {e}"),
        ),
    }
}

/// Lists assets in a workspace. Requires `Viewer` role.
pub async fn list_assets(
    State(state): State<AppState>,
    Path(workspace_id): Path<Uuid>,
    Query(params): Query<ListAssetsQuery>,
    request: axum::http::Request<axum::body::Body>,
) -> axum::response::Response {
    let Ok(user) = extract_current_user(request.extensions()) else {
        return error_response(StatusCode::UNAUTHORIZED, "not authenticated");
    };

    let Ok(pool) = state.require_db() else {
        return error_response(StatusCode::SERVICE_UNAVAILABLE, "database not configured");
    };

    if let Err(resp) = require_role(pool, workspace_id, user.id, WorkspaceRole::Viewer).await {
        return resp;
    }

    let limit = params.limit.unwrap_or(50).clamp(1, 200);
    let offset = params.offset.unwrap_or(0).max(0);

    match selean_db::queries::assets::list_assets(pool, workspace_id, limit, offset).await {
        Ok(assets) => {
            let responses: Vec<AssetResponse> =
                assets.iter().map(AssetResponse::from_asset).collect();
            Json(responses).into_response()
        }
        Err(e) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("database error: {e}"),
        ),
    }
}

/// Deletes an asset by ID. Requires `Editor` role.
pub async fn delete_asset(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    request: axum::http::Request<axum::body::Body>,
) -> axum::response::Response {
    let Ok(user) = extract_current_user(request.extensions()) else {
        return error_response(StatusCode::UNAUTHORIZED, "not authenticated");
    };

    let Ok(pool) = state.require_db() else {
        return error_response(StatusCode::SERVICE_UNAVAILABLE, "database not configured");
    };

    // Look up the asset to get workspace_id for RBAC.
    let asset = match selean_db::queries::assets::get_asset(pool, id).await {
        Ok(a) => a,
        Err(selean_db::DbError::NotFound(_)) => {
            return error_response(StatusCode::NOT_FOUND, "asset not found");
        }
        Err(e) => {
            return error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                &format!("database error: {e}"),
            );
        }
    };

    if let Err(resp) = require_role(pool, asset.workspace_id, user.id, WorkspaceRole::Editor).await
    {
        return resp;
    }

    // Delete from database first.
    let storage_key = match selean_db::queries::assets::delete_asset(pool, id).await {
        Ok(key) => key,
        Err(e) => {
            return error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                &format!("database error: {e}"),
            );
        }
    };

    // Delete from storage. Log but don't fail if cleanup fails.
    if let Err(e) = state.storage.delete(&storage_key).await {
        tracing::error!("failed to delete storage object {storage_key}: {e}");
    }

    StatusCode::NO_CONTENT.into_response()
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_filename_strips_slashes() {
        assert_eq!(sanitize_filename("path/to/file.png"), "pathtofile.png");
        assert_eq!(sanitize_filename("path\\to\\file.png"), "pathtofile.png");
    }

    #[test]
    fn sanitize_filename_replaces_dots() {
        // Slashes stripped first: "......etcpasswd", then ".." -> "_": "___etcpasswd"
        assert_eq!(sanitize_filename("../../../etc/passwd"), "___etcpasswd");
    }

    #[test]
    fn sanitize_filename_truncates_long_names() {
        let long_name = "a".repeat(300);
        let sanitized = sanitize_filename(&long_name);
        assert_eq!(sanitized.len(), 255);
    }

    #[test]
    fn sanitize_filename_preserves_normal_names() {
        assert_eq!(sanitize_filename("photo.png"), "photo.png");
        assert_eq!(
            sanitize_filename("my-document (1).pdf"),
            "my-document (1).pdf"
        );
    }

    #[test]
    fn content_type_allowlist_accepts_valid() {
        assert!(is_allowed_content_type("image/png"));
        assert!(is_allowed_content_type("image/jpeg"));
        assert!(is_allowed_content_type("image/webp"));
        assert!(is_allowed_content_type("image/svg+xml"));
        assert!(is_allowed_content_type("font/ttf"));
        assert!(is_allowed_content_type("font/otf"));
        assert!(is_allowed_content_type("font/woff"));
        assert!(is_allowed_content_type("font/woff2"));
        assert!(is_allowed_content_type("application/pdf"));
    }

    #[test]
    fn content_type_allowlist_rejects_invalid() {
        assert!(!is_allowed_content_type("text/html"));
        assert!(!is_allowed_content_type("application/javascript"));
        assert!(!is_allowed_content_type("application/octet-stream"));
        assert!(!is_allowed_content_type(""));
    }

    #[test]
    fn storage_key_format() {
        let ws = Uuid::new_v4();
        let asset = Uuid::new_v4();
        let filename = "photo.png";
        let key = format!("{ws}/{asset}/{filename}");
        assert!(key.starts_with(&ws.to_string()));
        assert!(key.ends_with("photo.png"));
        // Should have exactly 2 slashes.
        assert_eq!(key.matches('/').count(), 2);
    }

    #[test]
    fn asset_response_url_format() {
        let id = Uuid::new_v4();
        let asset = selean_db::models::asset::Asset {
            id,
            workspace_id: Uuid::new_v4(),
            filename: "test.png".to_string(),
            content_type: "image/png".to_string(),
            size_bytes: 1024,
            storage_key: "key".to_string(),
            uploaded_by: Uuid::new_v4(),
            created_at: chrono::Utc::now(),
        };
        let resp = AssetResponse::from_asset(&asset);
        assert_eq!(resp.url, format!("/api/assets/{id}"));
        assert_eq!(resp.filename, "test.png");
        assert_eq!(resp.content_type, "image/png");
    }
}
