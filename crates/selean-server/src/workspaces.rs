//! Workspace CRUD and member management API routes.
//!
//! Provides workspace creation, listing, updating, deletion, and member
//! management with role-based access control and billing tier enforcement.

use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, patch, post},
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use selean_db::models::workspace::WorkspaceRole;

use crate::rbac::{extract_current_user, require_role, resolve_billing_tier};
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

fn db_error_response(err: &selean_db::DbError) -> axum::response::Response {
    match err {
        selean_db::DbError::NotFound(entity) => {
            error(StatusCode::NOT_FOUND, &format!("{entity} not found"))
        }
        _ => error(StatusCode::INTERNAL_SERVER_ERROR, &err.to_string()),
    }
}

#[allow(clippy::result_large_err)]
fn require_db(state: &AppState) -> Result<&sqlx::PgPool, axum::response::Response> {
    state
        .require_db()
        .map_err(|e| error(StatusCode::SERVICE_UNAVAILABLE, &e.to_string()))
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
        Err(resp) => return resp,
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

    if body.name.is_empty() {
        return error(StatusCode::BAD_REQUEST, "workspace name is required");
    }

    // Check how many workspaces the user already owns.
    // Free tier users get a limited number. We use free tier limits as default.
    let owned =
        match selean_db::queries::workspaces::count_user_workspaces(pool, current_user.id).await {
            Ok(c) => c,
            Err(e) => return db_error_response(&e),
        };

    // Apply a generous default limit (10 workspaces per user).
    if owned >= 10 {
        return error(
            StatusCode::FORBIDDEN,
            "workspace limit reached for your account",
        );
    }

    match selean_db::queries::workspaces::create_workspace(pool, &body.name, current_user.id).await
    {
        Ok(w) => (StatusCode::CREATED, Json(WorkspaceResponse::from(w))).into_response(),
        Err(e) => db_error_response(&e),
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
        Err(resp) => return resp,
    };
    let current_user = match extract_current_user(request.extensions()) {
        Ok(u) => u,
        Err(resp) => return resp,
    };

    match selean_db::queries::workspaces::list_user_workspaces(pool, current_user.id).await {
        Ok(workspaces) => {
            let responses: Vec<WorkspaceResponse> =
                workspaces.into_iter().map(Into::into).collect();
            Json(responses).into_response()
        }
        Err(e) => db_error_response(&e),
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
        Err(resp) => return resp,
    };
    let current_user = match extract_current_user(request.extensions()) {
        Ok(u) => u,
        Err(resp) => return resp,
    };

    if let Err(resp) = require_role(pool, id, current_user.id, WorkspaceRole::Viewer).await {
        return resp;
    }

    match selean_db::queries::workspaces::get_workspace(pool, id).await {
        Ok(w) => Json(WorkspaceResponse::from(w)).into_response(),
        Err(e) => db_error_response(&e),
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
        Err(resp) => return resp,
    };
    let current_user = match extract_current_user(request.extensions()) {
        Ok(u) => u,
        Err(resp) => return resp,
    };

    if let Err(resp) = require_role(pool, id, current_user.id, WorkspaceRole::Owner).await {
        return resp;
    }

    let body: UpdateWorkspaceRequest = match axum::Json::from_bytes(
        &axum::body::to_bytes(request.into_body(), 1024 * 1024)
            .await
            .unwrap_or_default(),
    ) {
        Ok(Json(b)) => b,
        Err(_) => return error(StatusCode::BAD_REQUEST, "invalid request body"),
    };

    if body.name.is_empty() {
        return error(StatusCode::BAD_REQUEST, "workspace name is required");
    }

    match selean_db::queries::workspaces::update_workspace_name(pool, id, &body.name).await {
        Ok(w) => Json(WorkspaceResponse::from(w)).into_response(),
        Err(e) => db_error_response(&e),
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
        Err(resp) => return resp,
    };
    let current_user = match extract_current_user(request.extensions()) {
        Ok(u) => u,
        Err(resp) => return resp,
    };

    if let Err(resp) = require_role(pool, id, current_user.id, WorkspaceRole::Owner).await {
        return resp;
    }

    match selean_db::queries::workspaces::delete_workspace(pool, id).await {
        Ok(()) => (StatusCode::OK, Json(serde_json::json!({ "ok": true }))).into_response(),
        Err(e) => db_error_response(&e),
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
        Err(resp) => return resp,
    };
    let current_user = match extract_current_user(request.extensions()) {
        Ok(u) => u,
        Err(resp) => return resp,
    };

    if let Err(resp) = require_role(pool, id, current_user.id, WorkspaceRole::Viewer).await {
        return resp;
    }

    match selean_db::queries::workspaces::list_members_with_users(pool, id).await {
        Ok(members) => {
            let responses: Vec<MemberResponse> = members.into_iter().map(Into::into).collect();
            Json(responses).into_response()
        }
        Err(e) => db_error_response(&e),
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
        Err(resp) => return resp,
    };
    let current_user = match extract_current_user(request.extensions()) {
        Ok(u) => u,
        Err(resp) => return resp,
    };

    if let Err(resp) = require_role(pool, id, current_user.id, WorkspaceRole::Admin).await {
        return resp;
    }

    let body: AddMemberRequest = match axum::Json::from_bytes(
        &axum::body::to_bytes(request.into_body(), 1024 * 1024)
            .await
            .unwrap_or_default(),
    ) {
        Ok(Json(b)) => b,
        Err(_) => return error(StatusCode::BAD_REQUEST, "invalid request body"),
    };

    // Parse the requested role.
    let Some(role) = WorkspaceRole::from_str_role(&body.role) else {
        return error(StatusCode::BAD_REQUEST, "invalid role");
    };

    if role == WorkspaceRole::Owner {
        return error(StatusCode::BAD_REQUEST, "cannot add a member as owner");
    }

    // Check billing tier member limit.
    if let Err(resp) = check_member_limit(pool, id).await {
        return resp;
    }

    // Look up user by email.
    let target_user = match selean_db::queries::users::find_user_by_email(pool, &body.email).await {
        Ok(Some(u)) => u,
        Ok(None) => return error(StatusCode::NOT_FOUND, "user not found with that email"),
        Err(e) => return db_error_response(&e),
    };

    match selean_db::queries::workspaces::add_member(pool, id, target_user.id, role).await {
        Ok(_) => (StatusCode::CREATED, Json(serde_json::json!({ "ok": true }))).into_response(),
        Err(e) => db_error_response(&e),
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
        Err(resp) => return resp,
    };
    let current_user = match extract_current_user(request.extensions()) {
        Ok(u) => u,
        Err(resp) => return resp,
    };

    let caller_role = match require_role(pool, id, current_user.id, WorkspaceRole::Admin).await {
        Ok(r) => r,
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

    let Some(new_role) = WorkspaceRole::from_str_role(&body.role) else {
        return error(StatusCode::BAD_REQUEST, "invalid role");
    };

    if let Err(resp) = validate_role_change(
        pool,
        id,
        current_user.id,
        target_user_id,
        caller_role,
        new_role,
    )
    .await
    {
        return resp;
    }

    match selean_db::queries::workspaces::update_member_role(pool, id, target_user_id, new_role)
        .await
    {
        Ok(_) => (StatusCode::OK, Json(serde_json::json!({ "ok": true }))).into_response(),
        Err(e) => db_error_response(&e),
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
        Err(resp) => return resp,
    };
    let current_user = match extract_current_user(request.extensions()) {
        Ok(u) => u,
        Err(resp) => return resp,
    };

    if let Err(resp) = require_role(pool, id, current_user.id, WorkspaceRole::Admin).await {
        return resp;
    }

    if current_user.id == target_user_id {
        return error(
            StatusCode::BAD_REQUEST,
            "cannot remove yourself; transfer ownership or leave",
        );
    }

    // Cannot remove the owner.
    let target_role =
        match selean_db::queries::workspaces::get_member_role(pool, id, target_user_id).await {
            Ok(Some(r)) => r,
            Ok(None) => return error(StatusCode::NOT_FOUND, "member not found"),
            Err(e) => return db_error_response(&e),
        };

    if target_role == WorkspaceRole::Owner {
        return error(StatusCode::FORBIDDEN, "cannot remove the workspace owner");
    }

    match selean_db::queries::workspaces::remove_member(pool, id, target_user_id).await {
        Ok(()) => (StatusCode::OK, Json(serde_json::json!({ "ok": true }))).into_response(),
        Err(e) => db_error_response(&e),
    }
}

// --- Internal helpers ---

/// Validates that a role change is permitted.
#[allow(clippy::result_large_err)]
async fn validate_role_change(
    pool: &sqlx::PgPool,
    workspace_id: Uuid,
    caller_id: Uuid,
    target_id: Uuid,
    caller_role: WorkspaceRole,
    new_role: WorkspaceRole,
) -> Result<(), axum::response::Response> {
    // Cannot change own role.
    if caller_id == target_id {
        return Err(error(
            StatusCode::BAD_REQUEST,
            "cannot change your own role",
        ));
    }

    // Check target's current role.
    let target_role = match selean_db::queries::workspaces::get_member_role(
        pool,
        workspace_id,
        target_id,
    )
    .await
    {
        Ok(Some(r)) => r,
        Ok(None) => return Err(error(StatusCode::NOT_FOUND, "member not found")),
        Err(e) => return Err(db_error_response(&e)),
    };

    // Cannot change the owner's role.
    if target_role == WorkspaceRole::Owner {
        return Err(error(
            StatusCode::FORBIDDEN,
            "cannot change the owner's role",
        ));
    }

    // Admin cannot promote to Owner.
    if caller_role == WorkspaceRole::Admin && new_role == WorkspaceRole::Owner {
        return Err(error(
            StatusCode::FORBIDDEN,
            "admins cannot promote to owner",
        ));
    }

    Ok(())
}

/// Checks if the workspace has room for another member under its billing tier.
#[allow(clippy::result_large_err)]
async fn check_member_limit(
    pool: &sqlx::PgPool,
    workspace_id: Uuid,
) -> Result<(), axum::response::Response> {
    let workspace = selean_db::queries::workspaces::get_workspace(pool, workspace_id)
        .await
        .map_err(|e| db_error_response(&e))?;

    let tier = resolve_billing_tier(&workspace.billing_tier)?;

    if let Some(max) = tier.max_members() {
        let current = selean_db::queries::workspaces::count_workspace_members(pool, workspace_id)
            .await
            .map_err(|e| db_error_response(&e))?;
        if current >= i64::from(max) {
            return Err(error(
                StatusCode::FORBIDDEN,
                &format!(
                    "workspace member limit reached ({max}) for {} tier",
                    tier.as_str()
                ),
            ));
        }
    }

    Ok(())
}
