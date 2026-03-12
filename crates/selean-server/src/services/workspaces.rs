//! Workspace service layer.
//!
//! Business logic for workspace CRUD and member management, extracted
//! from route handlers. Each method takes a `PgPool` and parameters,
//! performs authorization and billing checks, and returns domain models
//! or `ServiceError`.

use sqlx::PgPool;
use uuid::Uuid;

use selean_db::models::workspace::{MemberWithUser, Workspace, WorkspaceRole};

use super::billing::check_member_limit;
use super::errors::ServiceError;
use crate::rbac;

/// Maximum workspaces a single user can own.
const MAX_USER_WORKSPACES: i64 = 10;

/// Creates a new workspace. Any authenticated user can create workspaces,
/// subject to a per-user ownership limit.
///
/// # Errors
///
/// Returns `ServiceError::Forbidden` if the user has reached the workspace limit.
pub async fn create_workspace(
    pool: &PgPool,
    name: &str,
    user_id: Uuid,
) -> Result<Workspace, ServiceError> {
    if name.is_empty() {
        return Err(ServiceError::bad_request("workspace name is required"));
    }

    let owned = selean_db::queries::workspaces::count_user_workspaces(pool, user_id).await?;

    if owned >= MAX_USER_WORKSPACES {
        return Err(ServiceError::forbidden(
            "workspace limit reached for your account",
        ));
    }

    let workspace = selean_db::queries::workspaces::create_workspace(pool, name, user_id).await?;

    Ok(workspace)
}

/// Gets a workspace by ID. Requires membership (any role).
///
/// # Errors
///
/// Returns `ServiceError` on permission or database failures.
pub async fn get_workspace(
    pool: &PgPool,
    workspace_id: Uuid,
    user_id: Uuid,
) -> Result<Workspace, ServiceError> {
    rbac::require_role(pool, workspace_id, user_id, WorkspaceRole::Viewer)
        .await
        .map_err(|_| ServiceError::forbidden("insufficient permissions"))?;

    let workspace = selean_db::queries::workspaces::get_workspace(pool, workspace_id).await?;

    Ok(workspace)
}

/// Lists all workspaces the user is a member of.
///
/// # Errors
///
/// Returns `ServiceError` on database failures.
pub async fn list_workspaces(pool: &PgPool, user_id: Uuid) -> Result<Vec<Workspace>, ServiceError> {
    let workspaces = selean_db::queries::workspaces::list_user_workspaces(pool, user_id).await?;
    Ok(workspaces)
}

/// Updates a workspace name. Requires `Owner` role.
///
/// # Errors
///
/// Returns `ServiceError` on permission or database failures.
pub async fn update_workspace(
    pool: &PgPool,
    workspace_id: Uuid,
    name: &str,
    user_id: Uuid,
) -> Result<Workspace, ServiceError> {
    rbac::require_role(pool, workspace_id, user_id, WorkspaceRole::Owner)
        .await
        .map_err(|_| ServiceError::forbidden("insufficient permissions"))?;

    if name.is_empty() {
        return Err(ServiceError::bad_request("workspace name is required"));
    }

    let workspace =
        selean_db::queries::workspaces::update_workspace_name(pool, workspace_id, name).await?;

    Ok(workspace)
}

/// Deletes a workspace. Requires `Owner` role.
///
/// # Errors
///
/// Returns `ServiceError` on permission or database failures.
pub async fn delete_workspace(
    pool: &PgPool,
    workspace_id: Uuid,
    user_id: Uuid,
) -> Result<(), ServiceError> {
    rbac::require_role(pool, workspace_id, user_id, WorkspaceRole::Owner)
        .await
        .map_err(|_| ServiceError::forbidden("insufficient permissions"))?;

    selean_db::queries::workspaces::delete_workspace(pool, workspace_id).await?;

    Ok(())
}

/// Lists workspace members with user profile info. Requires membership
/// (any role).
///
/// # Errors
///
/// Returns `ServiceError` on permission or database failures.
pub async fn list_members(
    pool: &PgPool,
    workspace_id: Uuid,
    user_id: Uuid,
) -> Result<Vec<MemberWithUser>, ServiceError> {
    rbac::require_role(pool, workspace_id, user_id, WorkspaceRole::Viewer)
        .await
        .map_err(|_| ServiceError::forbidden("insufficient permissions"))?;

    let members =
        selean_db::queries::workspaces::list_members_with_users(pool, workspace_id).await?;

    Ok(members)
}

/// Adds a member to a workspace by email. Requires `Admin`+ role.
/// Checks billing tier member limit. Validates role constraints.
///
/// # Errors
///
/// Returns `ServiceError` on permission, billing, lookup, or database failures.
pub async fn add_member(
    pool: &PgPool,
    workspace_id: Uuid,
    email: &str,
    role_str: &str,
    caller_id: Uuid,
) -> Result<(), ServiceError> {
    rbac::require_role(pool, workspace_id, caller_id, WorkspaceRole::Admin)
        .await
        .map_err(|_| ServiceError::forbidden("insufficient permissions"))?;

    let role = WorkspaceRole::from_str_role(role_str)
        .ok_or_else(|| ServiceError::bad_request("invalid role"))?;

    if role == WorkspaceRole::Owner {
        return Err(ServiceError::bad_request("cannot add a member as owner"));
    }

    check_member_limit(pool, workspace_id).await?;

    let target_user = selean_db::queries::users::find_user_by_email(pool, email)
        .await?
        .ok_or_else(|| ServiceError::NotFound("user with that email".to_string()))?;

    selean_db::queries::workspaces::add_member(pool, workspace_id, target_user.id, role).await?;

    Ok(())
}

/// Updates a member's role. Requires `Admin`+ role. Validates role change
/// constraints: cannot change own role, cannot change owner's role,
/// admin cannot promote to owner.
///
/// # Errors
///
/// Returns `ServiceError` on permission or database failures.
pub async fn update_member_role(
    pool: &PgPool,
    workspace_id: Uuid,
    target_user_id: Uuid,
    role_str: &str,
    caller_id: Uuid,
) -> Result<(), ServiceError> {
    let caller_role = rbac::require_role(pool, workspace_id, caller_id, WorkspaceRole::Admin)
        .await
        .map_err(|_| ServiceError::forbidden("insufficient permissions"))?;

    let new_role = WorkspaceRole::from_str_role(role_str)
        .ok_or_else(|| ServiceError::bad_request("invalid role"))?;

    validate_role_change(
        pool,
        workspace_id,
        caller_id,
        target_user_id,
        caller_role,
        new_role,
    )
    .await?;

    selean_db::queries::workspaces::update_member_role(
        pool,
        workspace_id,
        target_user_id,
        new_role,
    )
    .await?;

    Ok(())
}

/// Removes a member from a workspace. Requires `Admin`+ role.
/// Cannot remove self. Cannot remove the owner.
///
/// # Errors
///
/// Returns `ServiceError` on permission or database failures.
pub async fn remove_member(
    pool: &PgPool,
    workspace_id: Uuid,
    target_user_id: Uuid,
    caller_id: Uuid,
) -> Result<(), ServiceError> {
    rbac::require_role(pool, workspace_id, caller_id, WorkspaceRole::Admin)
        .await
        .map_err(|_| ServiceError::forbidden("insufficient permissions"))?;

    if caller_id == target_user_id {
        return Err(ServiceError::bad_request(
            "cannot remove yourself; transfer ownership or leave",
        ));
    }

    let target_role =
        selean_db::queries::workspaces::get_member_role(pool, workspace_id, target_user_id)
            .await?
            .ok_or_else(|| ServiceError::NotFound("member".to_string()))?;

    if target_role == WorkspaceRole::Owner {
        return Err(ServiceError::forbidden("cannot remove the workspace owner"));
    }

    selean_db::queries::workspaces::remove_member(pool, workspace_id, target_user_id).await?;

    Ok(())
}

/// Validates that a role change is permitted.
async fn validate_role_change(
    pool: &PgPool,
    workspace_id: Uuid,
    caller_id: Uuid,
    target_id: Uuid,
    caller_role: WorkspaceRole,
    new_role: WorkspaceRole,
) -> Result<(), ServiceError> {
    if caller_id == target_id {
        return Err(ServiceError::bad_request("cannot change your own role"));
    }

    let target_role =
        selean_db::queries::workspaces::get_member_role(pool, workspace_id, target_id)
            .await?
            .ok_or_else(|| ServiceError::NotFound("member".to_string()))?;

    if target_role == WorkspaceRole::Owner {
        return Err(ServiceError::forbidden("cannot change the owner's role"));
    }

    if caller_role == WorkspaceRole::Admin && new_role == WorkspaceRole::Owner {
        return Err(ServiceError::forbidden("admins cannot promote to owner"));
    }

    Ok(())
}
