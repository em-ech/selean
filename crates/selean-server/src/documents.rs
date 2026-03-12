//! Document management API routes.
//!
//! Provides CRUD operations for documents and document versions,
//! backed by `PostgreSQL` via `selean-db`. All routes enforce RBAC
//! via workspace membership checks.
//!
//! Handlers are thin: extract request, call service, format response.

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::rbac::extract_current_user;
use crate::services::errors::require_db;
use crate::state::AppState;

/// Creates the document management router.
///
/// All routes require a database connection. If the database is not
/// configured, the health check will report it and these routes will
/// return 503.
pub fn document_routes() -> Router<AppState> {
    Router::new()
        .route("/api/documents", post(create_document))
        .route("/api/documents/{id}", get(get_document))
        .route(
            "/api/workspaces/{workspace_id}/documents",
            get(list_documents),
        )
        .route(
            "/api/documents/{id}/versions",
            post(save_version).get(list_versions),
        )
        .route(
            "/api/documents/{id}/versions/latest",
            get(get_latest_version),
        )
}

/// Request body for creating a document.
#[derive(Debug, Deserialize)]
struct CreateDocumentRequest {
    workspace_id: Uuid,
    name: String,
}

/// Response for document metadata.
#[derive(Debug, Serialize)]
struct DocumentResponse {
    id: Uuid,
    workspace_id: Uuid,
    name: String,
    created_by: Uuid,
    created_at: String,
    updated_at: String,
}

impl From<selean_db::models::document::Document> for DocumentResponse {
    fn from(doc: selean_db::models::document::Document) -> Self {
        Self {
            id: doc.id,
            workspace_id: doc.workspace_id,
            name: doc.name,
            created_by: doc.created_by,
            created_at: doc.created_at.to_rfc3339(),
            updated_at: doc.updated_at.to_rfc3339(),
        }
    }
}

/// Response for a document version.
#[derive(Debug, Serialize)]
struct VersionResponse {
    id: Uuid,
    document_id: Uuid,
    version: i32,
    created_by: Uuid,
    created_at: String,
    /// Included only when fetching a single version.
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<serde_json::Value>,
}

impl VersionResponse {
    fn from_version(v: selean_db::models::document::DocumentVersion, include_data: bool) -> Self {
        Self {
            id: v.id,
            document_id: v.document_id,
            version: v.version,
            created_by: v.created_by,
            created_at: v.created_at.to_rfc3339(),
            data: if include_data { Some(v.data) } else { None },
        }
    }
}

/// Query parameters for listing documents.
#[derive(Debug, Deserialize)]
struct ListParams {
    #[serde(default = "default_limit")]
    limit: i64,
    #[serde(default)]
    offset: i64,
}

fn default_limit() -> i64 {
    50
}

/// Request body for saving a version.
#[derive(Debug, Deserialize)]
struct SaveVersionRequest {
    data: serde_json::Value,
}

fn error(status: StatusCode, msg: &str) -> axum::response::Response {
    (status, Json(serde_json::json!({ "error": msg }))).into_response()
}

/// `POST /api/documents`
///
/// Requires `Editor`+ in the workspace. Checks billing tier document limit.
async fn create_document(
    State(state): State<AppState>,
    request: axum::extract::Request,
) -> axum::response::Response {
    let pool = match require_db(&state) {
        Ok(p) => p,
        Err(e) => return e.into_response(),
    };
    let current_user = match extract_current_user(request.extensions()) {
        Ok(u) => u,
        Err(resp) => return resp,
    };

    let req: CreateDocumentRequest = match axum::Json::from_bytes(
        &axum::body::to_bytes(request.into_body(), 1024 * 1024)
            .await
            .unwrap_or_default(),
    ) {
        Ok(Json(b)) => b,
        Err(_) => return error(StatusCode::BAD_REQUEST, "invalid request body"),
    };

    match crate::services::documents::create_document(
        pool,
        req.workspace_id,
        &req.name,
        current_user.id,
    )
    .await
    {
        Ok(doc) => (StatusCode::CREATED, Json(DocumentResponse::from(doc))).into_response(),
        Err(e) => e.into_response(),
    }
}

/// `GET /api/documents/{id}`
///
/// Requires `Viewer`+ in the document's workspace.
async fn get_document(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    request: axum::extract::Request,
) -> axum::response::Response {
    let pool = match require_db(&state) {
        Ok(p) => p,
        Err(e) => return e.into_response(),
    };
    let current_user = match extract_current_user(request.extensions()) {
        Ok(u) => u,
        Err(resp) => return resp,
    };

    match crate::services::documents::get_document(pool, id, current_user.id).await {
        Ok(doc) => Json(DocumentResponse::from(doc)).into_response(),
        Err(e) => e.into_response(),
    }
}

/// `GET /api/workspaces/{workspace_id}/documents`
///
/// Requires `Viewer`+ in the workspace.
async fn list_documents(
    State(state): State<AppState>,
    Path(workspace_id): Path<Uuid>,
    Query(params): Query<ListParams>,
    request: axum::extract::Request,
) -> axum::response::Response {
    let pool = match require_db(&state) {
        Ok(p) => p,
        Err(e) => return e.into_response(),
    };
    let current_user = match extract_current_user(request.extensions()) {
        Ok(u) => u,
        Err(resp) => return resp,
    };

    match crate::services::documents::list_documents(
        pool,
        workspace_id,
        current_user.id,
        params.limit,
        params.offset,
    )
    .await
    {
        Ok(docs) => {
            let responses: Vec<DocumentResponse> = docs.into_iter().map(Into::into).collect();
            Json(responses).into_response()
        }
        Err(e) => e.into_response(),
    }
}

/// `POST /api/documents/{id}/versions`
///
/// Requires `Editor`+ in the document's workspace.
async fn save_version(
    State(state): State<AppState>,
    Path(document_id): Path<Uuid>,
    request: axum::extract::Request,
) -> axum::response::Response {
    let pool = match require_db(&state) {
        Ok(p) => p,
        Err(e) => return e.into_response(),
    };
    let current_user = match extract_current_user(request.extensions()) {
        Ok(u) => u,
        Err(resp) => return resp,
    };

    let req: SaveVersionRequest = match axum::Json::from_bytes(
        &axum::body::to_bytes(request.into_body(), 1024 * 1024)
            .await
            .unwrap_or_default(),
    ) {
        Ok(Json(b)) => b,
        Err(_) => return error(StatusCode::BAD_REQUEST, "invalid request body"),
    };

    match crate::services::documents::save_version(pool, document_id, &req.data, current_user.id)
        .await
    {
        Ok(v) => (
            StatusCode::CREATED,
            Json(VersionResponse::from_version(v, false)),
        )
            .into_response(),
        Err(e) => e.into_response(),
    }
}

/// `GET /api/documents/{id}/versions/latest`
///
/// Requires `Viewer`+ in the document's workspace.
async fn get_latest_version(
    State(state): State<AppState>,
    Path(document_id): Path<Uuid>,
    request: axum::extract::Request,
) -> axum::response::Response {
    let pool = match require_db(&state) {
        Ok(p) => p,
        Err(e) => return e.into_response(),
    };
    let current_user = match extract_current_user(request.extensions()) {
        Ok(u) => u,
        Err(resp) => return resp,
    };

    match crate::services::documents::get_latest_version(pool, document_id, current_user.id).await {
        Ok(v) => Json(VersionResponse::from_version(v, true)).into_response(),
        Err(e) => e.into_response(),
    }
}

/// `GET /api/documents/{id}/versions`
///
/// Requires `Viewer`+ in the document's workspace.
async fn list_versions(
    State(state): State<AppState>,
    Path(document_id): Path<Uuid>,
    Query(params): Query<ListParams>,
    request: axum::extract::Request,
) -> axum::response::Response {
    let pool = match require_db(&state) {
        Ok(p) => p,
        Err(e) => return e.into_response(),
    };
    let current_user = match extract_current_user(request.extensions()) {
        Ok(u) => u,
        Err(resp) => return resp,
    };

    match crate::services::documents::list_versions(
        pool,
        document_id,
        current_user.id,
        params.limit,
    )
    .await
    {
        Ok(versions) => {
            let responses: Vec<VersionResponse> = versions
                .into_iter()
                .map(|v| VersionResponse::from_version(v, false))
                .collect();
            Json(responses).into_response()
        }
        Err(e) => e.into_response(),
    }
}
