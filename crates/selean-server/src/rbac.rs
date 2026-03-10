//! Role-based access control helpers for workspace authorization.

use axum::{Json, http::StatusCode, response::IntoResponse};
use sqlx::PgPool;
use uuid::Uuid;

use selean_db::models::billing::BillingTier;
use selean_db::models::workspace::WorkspaceRole;

use crate::auth::middleware::CurrentUser;

/// Checks that the user is a member of the workspace with at least the given role.
/// Returns the user's actual role on success.
///
/// # Errors
///
/// Returns an HTTP error response if the user is not a member, has
/// insufficient permissions, or the database query fails.
#[allow(clippy::result_large_err)]
pub async fn require_role(
    pool: &PgPool,
    workspace_id: Uuid,
    user_id: Uuid,
    minimum: WorkspaceRole,
) -> Result<WorkspaceRole, axum::response::Response> {
    let role = selean_db::queries::workspaces::get_member_role(pool, workspace_id, user_id)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
                .into_response()
        })?;

    let Some(role) = role else {
        return Err(forbidden("not a member of this workspace"));
    };

    if !role.has_at_least(minimum) {
        return Err(forbidden("insufficient permissions"));
    }

    Ok(role)
}

/// Resolves the workspace's billing tier from its `billing_tier` column string.
///
/// # Errors
///
/// Returns an HTTP error response if the tier string is not recognized.
#[allow(clippy::result_large_err)]
pub fn resolve_billing_tier(tier_str: &str) -> Result<BillingTier, axum::response::Response> {
    BillingTier::from_str_tier(tier_str).ok_or_else(|| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": "invalid billing tier" })),
        )
            .into_response()
    })
}

/// Extracts `CurrentUser` from request extensions.
///
/// # Errors
///
/// Returns an HTTP 401 response if no `CurrentUser` is present in the
/// request extensions.
#[allow(clippy::result_large_err)]
pub fn extract_current_user(
    extensions: &axum::http::Extensions,
) -> Result<CurrentUser, axum::response::Response> {
    extensions.get::<CurrentUser>().cloned().ok_or_else(|| {
        (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({ "error": "not authenticated" })),
        )
            .into_response()
    })
}

fn forbidden(msg: &str) -> axum::response::Response {
    (
        StatusCode::FORBIDDEN,
        Json(serde_json::json!({ "error": msg })),
    )
        .into_response()
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn resolve_billing_tier_valid() {
        for tier_str in ["free", "pro", "team", "enterprise"] {
            assert!(resolve_billing_tier(tier_str).is_ok());
        }
    }

    #[test]
    fn resolve_billing_tier_invalid() {
        assert!(resolve_billing_tier("unknown").is_err());
    }

    #[test]
    fn extract_current_user_missing() {
        let ext = axum::http::Extensions::new();
        assert!(extract_current_user(&ext).is_err());
    }

    #[test]
    fn extract_current_user_present() {
        let mut ext = axum::http::Extensions::new();
        ext.insert(CurrentUser { id: Uuid::new_v4() });
        let user = extract_current_user(&ext).expect("user present");
        assert!(!user.id.is_nil());
    }
}
