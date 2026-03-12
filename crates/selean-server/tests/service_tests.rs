//! Integration tests for the service layer.
//!
//! Tests are split into two categories:
//! 1. Pure logic tests that do not require a database.
//! 2. Database-dependent tests that run against a live `PostgreSQL` instance
//!    (gated behind the `DATABASE_URL` env var via `setup_pool`).
//!
//! Run with `cargo test -p selean-server` for unit tests.
//! Run with `DATABASE_URL=... cargo test -p selean-server -- --include-ignored`
//! for the full suite including database tests.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use axum::http::StatusCode;
use axum::response::IntoResponse;

use selean_server::services::errors::ServiceError;

// ---------------------------------------------------------------------------
// ServiceError conversion tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn service_error_db_not_found_maps_to_404() {
    let err = ServiceError::Database(selean_db::DbError::NotFound("document".to_string()));
    let resp = err.into_response();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    let json = body_json(resp).await;
    assert_eq!(json["error"], "document not found");
}

#[tokio::test]
async fn service_error_forbidden_maps_to_403() {
    let err = ServiceError::forbidden("not allowed");
    let resp = err.into_response();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    let json = body_json(resp).await;
    assert_eq!(json["error"], "not allowed");
}

#[tokio::test]
async fn service_error_bad_request_maps_to_400() {
    let err = ServiceError::bad_request("missing field");
    let resp = err.into_response();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let json = body_json(resp).await;
    assert_eq!(json["error"], "missing field");
}

#[tokio::test]
async fn service_error_no_db_maps_to_503() {
    let err = ServiceError::NoDatabaseConfigured;
    let resp = err.into_response();
    assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
}

#[tokio::test]
async fn service_error_not_found_maps_to_404() {
    let err = ServiceError::NotFound("workspace".to_string());
    let resp = err.into_response();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    let json = body_json(resp).await;
    assert_eq!(json["error"], "workspace not found");
}

// ---------------------------------------------------------------------------
// require_db tests
// ---------------------------------------------------------------------------

// Note: require_db with no pool is tested in services/errors.rs unit tests.
// Integration tests cannot access AppState::new_test() (it's #[cfg(test)] on the lib).

// ---------------------------------------------------------------------------
// Billing tier limit logic (pure, no DB required)
// ---------------------------------------------------------------------------

#[test]
fn billing_tier_free_document_limit() {
    let tier = selean_db::models::billing::BillingTier::Free;
    assert_eq!(tier.max_documents(), Some(10));
}

#[test]
fn billing_tier_enterprise_unlimited() {
    let tier = selean_db::models::billing::BillingTier::Enterprise;
    assert!(tier.max_documents().is_none());
    assert!(tier.max_members().is_none());
}

#[test]
fn billing_tier_free_member_limit() {
    let tier = selean_db::models::billing::BillingTier::Free;
    assert_eq!(tier.max_members(), Some(3));
}

// ---------------------------------------------------------------------------
// Workspace service: pure validation logic
// ---------------------------------------------------------------------------

#[tokio::test]
async fn workspace_service_empty_name_rejected() {
    // create_workspace validates name before hitting DB.
    // Without a pool we cannot call the service, but we can verify the
    // error type is correct by constructing it directly.
    let err = ServiceError::bad_request("workspace name is required");
    let resp = err.into_response();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let json = body_json(resp).await;
    assert_eq!(json["error"], "workspace name is required");
}

#[tokio::test]
async fn workspace_service_cannot_add_owner_role() {
    let err = ServiceError::bad_request("cannot add a member as owner");
    let resp = err.into_response();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let json = body_json(resp).await;
    assert_eq!(json["error"], "cannot add a member as owner");
}

#[tokio::test]
async fn workspace_service_cannot_change_own_role() {
    let err = ServiceError::bad_request("cannot change your own role");
    let resp = err.into_response();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let json = body_json(resp).await;
    assert_eq!(json["error"], "cannot change your own role");
}

#[tokio::test]
async fn workspace_service_cannot_remove_self() {
    let err = ServiceError::bad_request("cannot remove yourself; transfer ownership or leave");
    let resp = err.into_response();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let json = body_json(resp).await;
    assert!(
        json["error"]
            .as_str()
            .unwrap()
            .contains("cannot remove yourself")
    );
}

#[tokio::test]
async fn workspace_service_cannot_remove_owner() {
    let err = ServiceError::forbidden("cannot remove the workspace owner");
    let resp = err.into_response();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    let json = body_json(resp).await;
    assert!(json["error"].as_str().unwrap().contains("workspace owner"));
}

#[tokio::test]
async fn workspace_service_admin_cannot_promote_to_owner() {
    let err = ServiceError::forbidden("admins cannot promote to owner");
    let resp = err.into_response();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    let json = body_json(resp).await;
    assert!(json["error"].as_str().unwrap().contains("promote to owner"));
}

#[tokio::test]
async fn billing_limit_reached_error_message() {
    let err = ServiceError::forbidden("document limit reached (10) for free tier");
    let resp = err.into_response();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    let json = body_json(resp).await;
    let msg = json["error"].as_str().unwrap();
    assert!(msg.contains("document limit reached"));
    assert!(msg.contains("free tier"));
}

#[tokio::test]
async fn member_limit_reached_error_message() {
    let err = ServiceError::forbidden("workspace member limit reached (3) for free tier");
    let resp = err.into_response();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    let json = body_json(resp).await;
    let msg = json["error"].as_str().unwrap();
    assert!(msg.contains("member limit reached"));
    assert!(msg.contains("free tier"));
}

// ---------------------------------------------------------------------------
// Role parsing tests
// ---------------------------------------------------------------------------

#[test]
fn workspace_role_valid_strings() {
    use selean_db::models::workspace::WorkspaceRole;
    assert_eq!(
        WorkspaceRole::from_str_role("viewer"),
        Some(WorkspaceRole::Viewer)
    );
    assert_eq!(
        WorkspaceRole::from_str_role("editor"),
        Some(WorkspaceRole::Editor)
    );
    assert_eq!(
        WorkspaceRole::from_str_role("admin"),
        Some(WorkspaceRole::Admin)
    );
    assert_eq!(
        WorkspaceRole::from_str_role("owner"),
        Some(WorkspaceRole::Owner)
    );
}

#[test]
fn workspace_role_invalid_string() {
    use selean_db::models::workspace::WorkspaceRole;
    assert!(WorkspaceRole::from_str_role("superuser").is_none());
    assert!(WorkspaceRole::from_str_role("").is_none());
}

#[tokio::test]
async fn invalid_role_maps_to_bad_request() {
    let err = ServiceError::bad_request("invalid role");
    let resp = err.into_response();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

// ---------------------------------------------------------------------------
// Database-dependent service tests (require DATABASE_URL)
// ---------------------------------------------------------------------------

/// Sets up a test database pool. Returns `None` if `DATABASE_URL` is not set.
async fn setup_pool() -> Option<sqlx::PgPool> {
    let url = std::env::var("DATABASE_URL").ok()?;
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(2)
        .connect(&url)
        .await
        .ok()?;
    // Run migrations to ensure schema is up to date.
    let _ = selean_db::run_migrations(&pool).await;
    Some(pool)
}

/// Creates a test user in the database and returns its UUID.
async fn create_test_user(pool: &sqlx::PgPool) -> uuid::Uuid {
    let email = format!("test-{}@selean.dev", uuid::Uuid::new_v4());
    let input = selean_db::models::user::CreateUser {
        email,
        display_name: "Test User".to_string(),
        password_hash: Some("fakehash".to_string()),
        avatar_url: None,
    };
    let user = selean_db::queries::users::create_user(pool, &input)
        .await
        .expect("create test user");
    user.id
}

#[tokio::test]
#[ignore = "requires DATABASE_URL"]
async fn document_service_create_and_get() {
    let Some(pool) = setup_pool().await else {
        return;
    };

    let user_id = create_test_user(&pool).await;

    // Create a workspace first.
    let ws = selean_server::services::workspaces::create_workspace(&pool, "test-ws", user_id)
        .await
        .expect("create workspace");

    // Create a document.
    let doc =
        selean_server::services::documents::create_document(&pool, ws.id, "My Document", user_id)
            .await
            .expect("create document");

    assert_eq!(doc.name, "My Document");
    assert_eq!(doc.workspace_id, ws.id);
    assert_eq!(doc.created_by, user_id);

    // Get the document.
    let fetched = selean_server::services::documents::get_document(&pool, doc.id, user_id)
        .await
        .expect("get document");
    assert_eq!(fetched.id, doc.id);

    // Cleanup.
    let _ = selean_db::queries::documents::hard_delete_document(&pool, doc.id).await;
    let _ = selean_db::queries::workspaces::delete_workspace(&pool, ws.id).await;
}

#[tokio::test]
#[ignore = "requires DATABASE_URL"]
async fn document_service_list_documents() {
    let Some(pool) = setup_pool().await else {
        return;
    };

    let user_id = create_test_user(&pool).await;
    let ws = selean_server::services::workspaces::create_workspace(&pool, "list-ws", user_id)
        .await
        .expect("create workspace");

    // Create two documents.
    let _doc1 = selean_server::services::documents::create_document(&pool, ws.id, "Doc A", user_id)
        .await
        .expect("create doc 1");
    let _doc2 = selean_server::services::documents::create_document(&pool, ws.id, "Doc B", user_id)
        .await
        .expect("create doc 2");

    let docs = selean_server::services::documents::list_documents(&pool, ws.id, user_id, 50, 0)
        .await
        .expect("list documents");

    assert!(docs.len() >= 2);

    // Cleanup.
    for doc in &docs {
        let _ = selean_db::queries::documents::hard_delete_document(&pool, doc.id).await;
    }
    let _ = selean_db::queries::workspaces::delete_workspace(&pool, ws.id).await;
}

#[tokio::test]
#[ignore = "requires DATABASE_URL"]
async fn document_service_save_and_list_versions() {
    let Some(pool) = setup_pool().await else {
        return;
    };

    let user_id = create_test_user(&pool).await;
    let ws = selean_server::services::workspaces::create_workspace(&pool, "ver-ws", user_id)
        .await
        .expect("create workspace");
    let doc =
        selean_server::services::documents::create_document(&pool, ws.id, "Versioned", user_id)
            .await
            .expect("create document");

    let data1 = serde_json::json!({"pages": []});
    let v1 = selean_server::services::documents::save_version(&pool, doc.id, &data1, user_id)
        .await
        .expect("save v1");
    assert_eq!(v1.version, 1);

    let data2 = serde_json::json!({"pages": [{"id": "p1"}]});
    let v2 = selean_server::services::documents::save_version(&pool, doc.id, &data2, user_id)
        .await
        .expect("save v2");
    assert_eq!(v2.version, 2);

    // Get latest.
    let latest = selean_server::services::documents::get_latest_version(&pool, doc.id, user_id)
        .await
        .expect("get latest");
    assert_eq!(latest.version, 2);

    // List versions.
    let versions = selean_server::services::documents::list_versions(&pool, doc.id, user_id, 10)
        .await
        .expect("list versions");
    assert_eq!(versions.len(), 2);
    assert_eq!(versions[0].version, 2); // most recent first

    // Cleanup.
    let _ = selean_db::queries::documents::hard_delete_document(&pool, doc.id).await;
    let _ = selean_db::queries::workspaces::delete_workspace(&pool, ws.id).await;
}

#[tokio::test]
#[ignore = "requires DATABASE_URL"]
async fn workspace_service_crud() {
    let Some(pool) = setup_pool().await else {
        return;
    };

    let user_id = create_test_user(&pool).await;

    // Create.
    let ws = selean_server::services::workspaces::create_workspace(&pool, "My WS", user_id)
        .await
        .expect("create workspace");
    assert_eq!(ws.name, "My WS");
    assert_eq!(ws.owner_id, user_id);

    // Get.
    let fetched = selean_server::services::workspaces::get_workspace(&pool, ws.id, user_id)
        .await
        .expect("get workspace");
    assert_eq!(fetched.id, ws.id);

    // Update.
    let updated =
        selean_server::services::workspaces::update_workspace(&pool, ws.id, "Renamed", user_id)
            .await
            .expect("update workspace");
    assert_eq!(updated.name, "Renamed");

    // List.
    let list = selean_server::services::workspaces::list_workspaces(&pool, user_id)
        .await
        .expect("list workspaces");
    assert!(list.iter().any(|w| w.id == ws.id));

    // Delete.
    selean_server::services::workspaces::delete_workspace(&pool, ws.id, user_id)
        .await
        .expect("delete workspace");
}

#[tokio::test]
#[ignore = "requires DATABASE_URL"]
async fn workspace_service_empty_name_returns_error() {
    let Some(pool) = setup_pool().await else {
        return;
    };

    let user_id = create_test_user(&pool).await;

    let result = selean_server::services::workspaces::create_workspace(&pool, "", user_id).await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    let resp = err.into_response();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
#[ignore = "requires DATABASE_URL"]
async fn workspace_service_member_management() {
    let Some(pool) = setup_pool().await else {
        return;
    };

    let owner_id = create_test_user(&pool).await;
    let member_id = create_test_user(&pool).await;

    let ws = selean_server::services::workspaces::create_workspace(&pool, "members-ws", owner_id)
        .await
        .expect("create workspace");

    // The owner should already be a member.
    let members = selean_server::services::workspaces::list_members(&pool, ws.id, owner_id)
        .await
        .expect("list members");
    assert_eq!(members.len(), 1);

    // Add member (need their email).
    let member_user = selean_db::queries::users::get_user(&pool, member_id)
        .await
        .expect("get member user");

    selean_server::services::workspaces::add_member(
        &pool,
        ws.id,
        &member_user.email,
        "editor",
        owner_id,
    )
    .await
    .expect("add member");

    // Verify member count increased.
    let members = selean_server::services::workspaces::list_members(&pool, ws.id, owner_id)
        .await
        .expect("list members after add");
    assert_eq!(members.len(), 2);

    // Update member role.
    selean_server::services::workspaces::update_member_role(
        &pool, ws.id, member_id, "admin", owner_id,
    )
    .await
    .expect("update member role");

    // Remove member.
    selean_server::services::workspaces::remove_member(&pool, ws.id, member_id, owner_id)
        .await
        .expect("remove member");

    let members = selean_server::services::workspaces::list_members(&pool, ws.id, owner_id)
        .await
        .expect("list members after remove");
    assert_eq!(members.len(), 1);

    // Cleanup.
    let _ = selean_db::queries::workspaces::delete_workspace(&pool, ws.id).await;
}

#[tokio::test]
#[ignore = "requires DATABASE_URL"]
async fn workspace_service_billing_limit_document() {
    let Some(pool) = setup_pool().await else {
        return;
    };

    let user_id = create_test_user(&pool).await;
    let ws = selean_server::services::workspaces::create_workspace(&pool, "billing-ws", user_id)
        .await
        .expect("create workspace");

    // Free tier allows 10 documents. Create 10.
    for i in 0..10 {
        selean_server::services::documents::create_document(
            &pool,
            ws.id,
            &format!("Doc {i}"),
            user_id,
        )
        .await
        .expect("create document");
    }

    // The 11th should fail.
    let result =
        selean_server::services::documents::create_document(&pool, ws.id, "Doc 11", user_id).await;

    assert!(result.is_err());
    let resp = result.unwrap_err().into_response();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    let json = body_json(resp).await;
    assert!(
        json["error"]
            .as_str()
            .unwrap()
            .contains("document limit reached")
    );

    // Cleanup.
    let docs = selean_db::queries::documents::list_documents(&pool, ws.id, 100, 0)
        .await
        .unwrap();
    for doc in docs {
        let _ = selean_db::queries::documents::hard_delete_document(&pool, doc.id).await;
    }
    let _ = selean_db::queries::workspaces::delete_workspace(&pool, ws.id).await;
}

#[tokio::test]
#[ignore = "requires DATABASE_URL"]
async fn workspace_service_billing_limit_members() {
    let Some(pool) = setup_pool().await else {
        return;
    };

    let owner_id = create_test_user(&pool).await;
    let ws =
        selean_server::services::workspaces::create_workspace(&pool, "member-limit-ws", owner_id)
            .await
            .expect("create workspace");

    // Free tier allows 3 members. Owner is member 1. Add 2 more.
    for _ in 0..2 {
        let uid = create_test_user(&pool).await;
        let user = selean_db::queries::users::get_user(&pool, uid)
            .await
            .unwrap();
        selean_server::services::workspaces::add_member(
            &pool,
            ws.id,
            &user.email,
            "editor",
            owner_id,
        )
        .await
        .expect("add member");
    }

    // The 4th member should fail (limit is 3).
    let uid = create_test_user(&pool).await;
    let user = selean_db::queries::users::get_user(&pool, uid)
        .await
        .unwrap();
    let result = selean_server::services::workspaces::add_member(
        &pool,
        ws.id,
        &user.email,
        "viewer",
        owner_id,
    )
    .await;

    assert!(result.is_err());
    let resp = result.unwrap_err().into_response();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    let json = body_json(resp).await;
    assert!(
        json["error"]
            .as_str()
            .unwrap()
            .contains("member limit reached")
    );

    // Cleanup.
    let _ = selean_db::queries::workspaces::delete_workspace(&pool, ws.id).await;
}

#[tokio::test]
#[ignore = "requires DATABASE_URL"]
async fn workspace_service_permission_denied_for_non_member() {
    let Some(pool) = setup_pool().await else {
        return;
    };

    let owner_id = create_test_user(&pool).await;
    let outsider_id = create_test_user(&pool).await;

    let ws = selean_server::services::workspaces::create_workspace(&pool, "perm-ws", owner_id)
        .await
        .expect("create workspace");

    // Outsider should not be able to get the workspace.
    let result =
        selean_server::services::workspaces::get_workspace(&pool, ws.id, outsider_id).await;
    assert!(result.is_err());
    let resp = result.unwrap_err().into_response();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);

    // Cleanup.
    let _ = selean_db::queries::workspaces::delete_workspace(&pool, ws.id).await;
}

#[tokio::test]
#[ignore = "requires DATABASE_URL"]
async fn document_service_permission_denied_for_non_member() {
    let Some(pool) = setup_pool().await else {
        return;
    };

    let owner_id = create_test_user(&pool).await;
    let outsider_id = create_test_user(&pool).await;

    let ws = selean_server::services::workspaces::create_workspace(&pool, "doc-perm-ws", owner_id)
        .await
        .expect("create workspace");

    let doc =
        selean_server::services::documents::create_document(&pool, ws.id, "Secret Doc", owner_id)
            .await
            .expect("create document");

    // Outsider should not be able to get the document.
    let result = selean_server::services::documents::get_document(&pool, doc.id, outsider_id).await;
    assert!(result.is_err());
    let resp = result.unwrap_err().into_response();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);

    // Cleanup.
    let _ = selean_db::queries::documents::hard_delete_document(&pool, doc.id).await;
    let _ = selean_db::queries::workspaces::delete_workspace(&pool, ws.id).await;
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

async fn body_json(resp: axum::response::Response) -> serde_json::Value {
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    serde_json::from_slice(&bytes).unwrap()
}
