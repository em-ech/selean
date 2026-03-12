//! Billing tier limit checking.
//!
//! Provides a generic `check_resource_limit` function that replaces the
//! duplicated `check_document_limit` and `check_member_limit` helpers.

use sqlx::PgPool;
use uuid::Uuid;

use selean_db::models::billing::BillingTier;

use super::errors::ServiceError;

/// Checks whether a workspace has room for another resource under its
/// billing tier.
///
/// `count_fn` fetches the current count of the resource.
/// `limit_fn` extracts the tier's limit for that resource type.
/// `resource_name` is used in the error message (e.g. "document", "member").
///
/// # Errors
///
/// Returns `ServiceError::Forbidden` if the limit is reached.
/// Returns `ServiceError::Database` if a query fails.
pub async fn check_resource_limit<F>(
    pool: &PgPool,
    workspace_id: Uuid,
    count_fn: impl std::future::Future<Output = Result<i64, selean_db::DbError>>,
    limit_fn: F,
    resource_name: &str,
) -> Result<(), ServiceError>
where
    F: FnOnce(&BillingTier) -> Option<u32>,
{
    let workspace = selean_db::queries::workspaces::get_workspace(pool, workspace_id).await?;

    let tier = BillingTier::from_str_tier(&workspace.billing_tier).ok_or_else(|| {
        ServiceError::Database(selean_db::DbError::NotFound(
            "valid billing tier".to_string(),
        ))
    })?;

    if let Some(max) = limit_fn(&tier) {
        let current = count_fn.await?;
        if current >= i64::from(max) {
            return Err(ServiceError::forbidden(format!(
                "{resource_name} limit reached ({max}) for {} tier",
                tier.as_str()
            )));
        }
    }

    Ok(())
}

/// Checks if the workspace has room for another document under its billing tier.
///
/// # Errors
///
/// Returns `ServiceError::Forbidden` if the document limit is reached.
pub async fn check_document_limit(pool: &PgPool, workspace_id: Uuid) -> Result<(), ServiceError> {
    check_resource_limit(
        pool,
        workspace_id,
        selean_db::queries::workspaces::count_workspace_documents(pool, workspace_id),
        BillingTier::max_documents,
        "document",
    )
    .await
}

/// Checks if the workspace has room for another member under its billing tier.
///
/// # Errors
///
/// Returns `ServiceError::Forbidden` if the member limit is reached.
pub async fn check_member_limit(pool: &PgPool, workspace_id: Uuid) -> Result<(), ServiceError> {
    check_resource_limit(
        pool,
        workspace_id,
        selean_db::queries::workspaces::count_workspace_members(pool, workspace_id),
        BillingTier::max_members,
        "workspace member",
    )
    .await
}
