//! Workspace CRUD and member management API routes.
//!
//! Provides workspace creation, listing, updating, deletion, and member
//! management with role-based access control and billing tier enforcement.
//!
//! Handlers are thin: extract request, call service, format response.

use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, patch, post},
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::rbac::extract_current_user;
use crate::services::errors::require_db;
use crate::state::AppState;

/// Creates the workspace management router.
pub fn workspace_routes() -> Router<AppState> {
    Router::new()
        .route(
            "/api/workspaces",
            post(create_workspace).get(list_workspaces),
        )
        .route(
            "/api/workspaces/{id}",
            get(get_workspace)
                .patch(update_workspace)
                .delete(delete_workspace),
        )
        .route(
            "/api/workspaces/{id}/members",
            get(list_members).post(add_member),
        )
        .route(
            "/api/workspaces/{id}/members/{user_id}",
            patch(update_member_role).delete(remove_member),
        )
}

// --- Request / Response types ---

#[derive(Debug, Deserialize)]
struct CreateWorkspaceRequest {
    name: String,
}

#[derive(Debug, Deserialize)]
struct UpdateWorkspaceRequest {
    name: String,
}

#[derive(Debug, Deserialize)]
struct AddMemberRequest {
    email: String,
    role: String,
}

#[derive(Debug, Deserialize)]
struct UpdateMemberRoleRequest {
    role: String,
}

#[derive(Debug, Serialize)]
struct WorkspaceResponse {
    id: Uuid,
    name: String,
    owner_id: Uuid,
    billing_tier: String,
    created_at: String,
    updated_at: String,
}

impl From<selean_db::models::workspace::Workspace> for WorkspaceResponse {
    fn from(w: selean_db::models::workspace::Workspace) -> Self {
        Self {
            id: w.id,
            name: w.name,
            owner_id: w.owner_id,
            billing_tier: w.billing_tier,
            created_at: w.created_at.to_rfc3339(),
            updated_at: w.updated_at.to_rfc3339(),
        }
    }
}

#[derive(Debug, Serialize)]
struct MemberResponse {
    workspace_id: Uuid,
    user_id: Uuid,
    email: String,
    display_name: String,
    role: String,
    joined_at: String,
}

impl From<selean_db::models::workspace::MemberWithUser> for MemberResponse {
    fn from(m: selean_db::models::workspace::MemberWithUser) -> Self {
        Self {
            workspace_id: m.workspace_id,
            user_id: m.user_id,
            email: m.email,
            display_name: m.display_name,
            role: m.role,
            joined_at: m.joined_at.to_rfc3339(),
        }
    }
}

// --- Helpers ---

fn error(status: StatusCode, msg: &str) -> axum::response::Response {
    (status, Json(serde_json::json!({ "error": msg }))).into_response()
}

// --- Handlers ---

/// `POST /api/workspaces`
///
/// Creates a new workspace. Any authenticated user can create workspaces,
/// subject to the free tier limit on owned workspace count.
async fn create_workspace(
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

    let body: CreateWorkspaceRequest = match axum::Json::from_bytes(
        &axum::body::to_bytes(request.into_body(), 1024 * 1024)
            .await
            .unwrap_or_default(),
    ) {
        Ok(Json(b)) => b,
        Err(_) => return error(StatusCode::BAD_REQUEST, "invalid request body"),
    };

    match crate::services::workspaces::create_workspace(pool, &body.name, current_user.id).await {
        Ok(w) => (StatusCode::CREATED, Json(WorkspaceResponse::from(w))).into_response(),
        Err(e) => e.into_response(),
    }
}

/// `GET /api/workspaces`
///
/// Lists all workspaces the current user is a member of.
async fn list_workspaces(
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

    match crate::services::workspaces::list_workspaces(pool, current_user.id).await {
        Ok(workspaces) => {
            let responses: Vec<WorkspaceResponse> =
                workspaces.into_iter().map(Into::into).collect();
            Json(responses).into_response()
        }
        Err(e) => e.into_response(),
    }
}

/// `GET /api/workspaces/{id}`
///
/// Returns workspace details. Requires membership (any role).
async fn get_workspace(
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

    match crate::services::workspaces::get_workspace(pool, id, current_user.id).await {
        Ok(w) => Json(WorkspaceResponse::from(w)).into_response(),
        Err(e) => e.into_response(),
    }
}

/// `PATCH /api/workspaces/{id}`
///
/// Updates workspace name. Requires `Owner` role.
async fn update_workspace(
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

    let body: UpdateWorkspaceRequest = match axum::Json::from_bytes(
        &axum::body::to_bytes(request.into_body(), 1024 * 1024)
            .await
            .unwrap_or_default(),
    ) {
        Ok(Json(b)) => b,
        Err(_) => return error(StatusCode::BAD_REQUEST, "invalid request body"),
    };

    match crate::services::workspaces::update_workspace(pool, id, &body.name, current_user.id).await
    {
        Ok(w) => Json(WorkspaceResponse::from(w)).into_response(),
        Err(e) => e.into_response(),
    }
}

/// `DELETE /api/workspaces/{id}`
///
/// Hard-deletes a workspace. Requires `Owner` role.
async fn delete_workspace(
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

    match crate::services::workspaces::delete_workspace(pool, id, current_user.id).await {
        Ok(()) => (StatusCode::OK, Json(serde_json::json!({ "ok": true }))).into_response(),
        Err(e) => e.into_response(),
    }
}

/// `GET /api/workspaces/{id}/members`
///
/// Lists workspace members with user info. Requires membership (any role).
async fn list_members(
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

    match crate::services::workspaces::list_members(pool, id, current_user.id).await {
        Ok(members) => {
            let responses: Vec<MemberResponse> = members.into_iter().map(Into::into).collect();
            Json(responses).into_response()
        }
        Err(e) => e.into_response(),
    }
}

/// `POST /api/workspaces/{id}/members`
///
/// Adds a member to the workspace. Requires `Admin`+ role. Checks billing
/// tier member limit. Looks up user by email.
async fn add_member(
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

    let body: AddMemberRequest = match axum::Json::from_bytes(
        &axum::body::to_bytes(request.into_body(), 1024 * 1024)
            .await
            .unwrap_or_default(),
    ) {
        Ok(Json(b)) => b,
        Err(_) => return error(StatusCode::BAD_REQUEST, "invalid request body"),
    };

    match crate::services::workspaces::add_member(
        pool,
        id,
        &body.email,
        &body.role,
        current_user.id,
    )
    .await
    {
        Ok(()) => (StatusCode::CREATED, Json(serde_json::json!({ "ok": true }))).into_response(),
        Err(e) => e.into_response(),
    }
}

/// `PATCH /api/workspaces/{id}/members/{user_id}`
///
/// Updates a member's role. Requires `Admin`+ role. Cannot change owner's
/// role, cannot change own role, admin cannot promote to owner.
async fn update_member_role(
    State(state): State<AppState>,
    Path((id, target_user_id)): Path<(Uuid, Uuid)>,
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

    let body: UpdateMemberRoleRequest = match axum::Json::from_bytes(
        &axum::body::to_bytes(request.into_body(), 1024 * 1024)
            .await
            .unwrap_or_default(),
    ) {
        Ok(Json(b)) => b,
        Err(_) => return error(StatusCode::BAD_REQUEST, "invalid request body"),
    };

    match crate::services::workspaces::update_member_role(
        pool,
        id,
        target_user_id,
        &body.role,
        current_user.id,
    )
    .await
    {
        Ok(()) => (StatusCode::OK, Json(serde_json::json!({ "ok": true }))).into_response(),
        Err(e) => e.into_response(),
    }
}

/// `DELETE /api/workspaces/{id}/members/{user_id}`
///
/// Removes a member from the workspace. Requires `Admin`+ role.
/// Cannot remove the owner. Cannot remove self.
async fn remove_member(
    State(state): State<AppState>,
    Path((id, target_user_id)): Path<(Uuid, Uuid)>,
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

    match crate::services::workspaces::remove_member(pool, id, target_user_id, current_user.id)
        .await
    {
        Ok(()) => (StatusCode::OK, Json(serde_json::json!({ "ok": true }))).into_response(),
        Err(e) => e.into_response(),
    }
}
