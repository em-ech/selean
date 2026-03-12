//! Centralized service error types and HTTP response helpers.
//!
//! Replaces the duplicated `db_error_response`, `error`, and `require_db`
//! helpers that were copy-pasted between `documents.rs` and `workspaces.rs`.

use axum::{Json, http::StatusCode, response::IntoResponse};

use crate::state::AppState;

/// A service-layer error that can be converted to an HTTP response.
#[derive(Debug, thiserror::Error)]
pub enum ServiceError {
    /// A database query failed.
    #[error("{0}")]
    Database(#[from] selean_db::DbError),

    /// The requested entity was not found.
    #[error("{0} not found")]
    NotFound(String),

    /// The caller lacks the required permission.
    #[error("{0}")]
    Forbidden(String),

    /// The request was malformed.
    #[error("{0}")]
    BadRequest(String),

    /// The database is not configured.
    #[error("database not configured")]
    NoDatabaseConfigured,
}

impl ServiceError {
    /// Creates a `Forbidden` error with the given message.
    pub fn forbidden(msg: impl Into<String>) -> Self {
        Self::Forbidden(msg.into())
    }

    /// Creates a `BadRequest` error with the given message.
    pub fn bad_request(msg: impl Into<String>) -> Self {
        Self::BadRequest(msg.into())
    }
}

impl IntoResponse for ServiceError {
    fn into_response(self) -> axum::response::Response {
        let (status, message) = match &self {
            ServiceError::Database(selean_db::DbError::NotFound(entity)) => {
                (StatusCode::NOT_FOUND, format!("{entity} not found"))
            }
            ServiceError::Database(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
            ServiceError::NotFound(entity) => {
                (StatusCode::NOT_FOUND, format!("{entity} not found"))
            }
            ServiceError::Forbidden(msg) => (StatusCode::FORBIDDEN, msg.clone()),
            ServiceError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg.clone()),
            ServiceError::NoDatabaseConfigured => (
                StatusCode::SERVICE_UNAVAILABLE,
                "DATABASE_URL not configured; database features are unavailable".to_string(),
            ),
        };
        (status, Json(serde_json::json!({ "error": message }))).into_response()
    }
}

/// Extracts the database pool from app state, returning a `ServiceError`
/// if the database is not configured.
pub fn require_db(state: &AppState) -> Result<&sqlx::PgPool, ServiceError> {
    state
        .require_db()
        .map_err(|_| ServiceError::NoDatabaseConfigured)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Response;

    async fn body_json(resp: Response<Body>) -> serde_json::Value {
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        serde_json::from_slice(&bytes).unwrap()
    }

    #[tokio::test]
    async fn service_error_not_found_from_db() {
        let err = ServiceError::Database(selean_db::DbError::NotFound("document".to_string()));
        let resp = err.into_response();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
        let json = body_json(resp).await;
        assert_eq!(json["error"], "document not found");
    }

    #[tokio::test]
    async fn service_error_forbidden() {
        let err = ServiceError::forbidden("insufficient permissions");
        let resp = err.into_response();
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
        let json = body_json(resp).await;
        assert_eq!(json["error"], "insufficient permissions");
    }

    #[tokio::test]
    async fn service_error_bad_request() {
        let err = ServiceError::bad_request("invalid role");
        let resp = err.into_response();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
        let json = body_json(resp).await;
        assert_eq!(json["error"], "invalid role");
    }

    #[tokio::test]
    async fn service_error_no_database() {
        let err = ServiceError::NoDatabaseConfigured;
        let resp = err.into_response();
        assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
    }

    #[test]
    fn require_db_returns_error_when_no_db() {
        let state = AppState::new_test();
        let result = require_db(&state);
        assert!(result.is_err());
    }
}
