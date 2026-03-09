//! Workspace and membership queries.

use sqlx::PgPool;
use uuid::Uuid;

use crate::DbError;
use crate::models::workspace::{Workspace, WorkspaceMember, WorkspaceRole};

/// Creates a workspace and adds the creator as owner.
///
/// # Errors
///
/// Returns an error if the query fails.
pub async fn create_workspace(
    pool: &PgPool,
    name: &str,
    owner_id: Uuid,
) -> Result<Workspace, DbError> {
    let id = Uuid::new_v4();

    let workspace = sqlx::query_as::<_, Workspace>(
        r"
        INSERT INTO workspaces (id, name, owner_id)
        VALUES ($1, $2, $3)
        RETURNING *
        ",
    )
    .bind(id)
    .bind(name)
    .bind(owner_id)
    .fetch_one(pool)
    .await?;

    // Add creator as owner member.
    sqlx::query(
        r"
        INSERT INTO workspace_members (workspace_id, user_id, role)
        VALUES ($1, $2, $3)
        ",
    )
    .bind(id)
    .bind(owner_id)
    .bind(WorkspaceRole::Owner.as_str())
    .execute(pool)
    .await?;

    Ok(workspace)
}

/// Gets a workspace by ID.
///
/// # Errors
///
/// Returns `NotFound` if the workspace does not exist.
pub async fn get_workspace(pool: &PgPool, id: Uuid) -> Result<Workspace, DbError> {
    sqlx::query_as::<_, Workspace>("SELECT * FROM workspaces WHERE id = $1")
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| DbError::NotFound("workspace".to_string()))
}

/// Lists all workspaces a user is a member of.
///
/// # Errors
///
/// Returns an error if the query fails.
pub async fn list_user_workspaces(pool: &PgPool, user_id: Uuid) -> Result<Vec<Workspace>, DbError> {
    let workspaces = sqlx::query_as::<_, Workspace>(
        r"
        SELECT w.*
        FROM workspaces w
        JOIN workspace_members wm ON w.id = wm.workspace_id
        WHERE wm.user_id = $1
        ORDER BY w.created_at DESC
        ",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    Ok(workspaces)
}

/// Updates a workspace name.
///
/// # Errors
///
/// Returns `NotFound` if the workspace does not exist.
pub async fn update_workspace_name(
    pool: &PgPool,
    id: Uuid,
    name: &str,
) -> Result<Workspace, DbError> {
    sqlx::query_as::<_, Workspace>(
        r"
        UPDATE workspaces SET name = $2, updated_at = now()
        WHERE id = $1
        RETURNING *
        ",
    )
    .bind(id)
    .bind(name)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| DbError::NotFound("workspace".to_string()))
}

/// Adds a member to a workspace.
///
/// # Errors
///
/// Returns an error if the membership already exists or the query fails.
pub async fn add_member(
    pool: &PgPool,
    workspace_id: Uuid,
    user_id: Uuid,
    role: WorkspaceRole,
) -> Result<WorkspaceMember, DbError> {
    let member = sqlx::query_as::<_, WorkspaceMember>(
        r"
        INSERT INTO workspace_members (workspace_id, user_id, role)
        VALUES ($1, $2, $3)
        RETURNING *
        ",
    )
    .bind(workspace_id)
    .bind(user_id)
    .bind(role.as_str())
    .fetch_one(pool)
    .await?;

    Ok(member)
}

/// Removes a member from a workspace.
///
/// # Errors
///
/// Returns `NotFound` if the membership does not exist.
pub async fn remove_member(
    pool: &PgPool,
    workspace_id: Uuid,
    user_id: Uuid,
) -> Result<(), DbError> {
    let result =
        sqlx::query("DELETE FROM workspace_members WHERE workspace_id = $1 AND user_id = $2")
            .bind(workspace_id)
            .bind(user_id)
            .execute(pool)
            .await?;

    if result.rows_affected() == 0 {
        return Err(DbError::NotFound("workspace member".to_string()));
    }
    Ok(())
}

/// Updates a member's role.
///
/// # Errors
///
/// Returns `NotFound` if the membership does not exist.
pub async fn update_member_role(
    pool: &PgPool,
    workspace_id: Uuid,
    user_id: Uuid,
    role: WorkspaceRole,
) -> Result<WorkspaceMember, DbError> {
    sqlx::query_as::<_, WorkspaceMember>(
        r"
        UPDATE workspace_members SET role = $3
        WHERE workspace_id = $1 AND user_id = $2
        RETURNING *
        ",
    )
    .bind(workspace_id)
    .bind(user_id)
    .bind(role.as_str())
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| DbError::NotFound("workspace member".to_string()))
}

/// Gets a user's role in a workspace, if they are a member.
///
/// # Errors
///
/// Returns an error if the query fails.
pub async fn get_member_role(
    pool: &PgPool,
    workspace_id: Uuid,
    user_id: Uuid,
) -> Result<Option<WorkspaceRole>, DbError> {
    let row: Option<(String,)> = sqlx::query_as(
        "SELECT role FROM workspace_members WHERE workspace_id = $1 AND user_id = $2",
    )
    .bind(workspace_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await?;

    Ok(row.and_then(|(r,)| WorkspaceRole::from_str_role(&r)))
}

/// Lists all members of a workspace.
///
/// # Errors
///
/// Returns an error if the query fails.
pub async fn list_members(
    pool: &PgPool,
    workspace_id: Uuid,
) -> Result<Vec<WorkspaceMember>, DbError> {
    let members = sqlx::query_as::<_, WorkspaceMember>(
        "SELECT * FROM workspace_members WHERE workspace_id = $1 ORDER BY joined_at",
    )
    .bind(workspace_id)
    .fetch_all(pool)
    .await?;

    Ok(members)
}
