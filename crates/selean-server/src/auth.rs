//! Optional token-based authentication middleware.
//!
//! When `SELEAN_AUTH_SECRET` is set, all API endpoints (except `/api/health`
//! and `/api/auth/token`) require an `Authorization: Bearer <secret>` header.
//! When the env var is unset, auth is disabled (open access for local dev).

use std::sync::Arc;

use axum::{
    Json,
    extract::{Request, State},
    http::{HeaderMap, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};

/// Shared auth configuration.
#[derive(Clone, Debug)]
pub struct AuthConfig {
    /// The shared secret. `None` means auth is disabled.
    secret: Option<Arc<str>>,
}

impl AuthConfig {
    /// Creates auth config from environment.
    ///
    /// If `SELEAN_AUTH_SECRET` is set and non-empty, auth is enabled.
    /// Otherwise, auth is disabled (all requests pass through).
    #[must_use]
    pub fn from_env() -> Self {
        let secret = std::env::var("SELEAN_AUTH_SECRET")
            .ok()
            .filter(|s| !s.is_empty())
            .map(Arc::from);
        Self { secret }
    }

    /// Creates auth config with a specific secret (for testing).
    #[must_use]
    pub fn with_secret(secret: &str) -> Self {
        Self {
            secret: Some(Arc::from(secret)),
        }
    }

    /// Creates auth config with auth disabled (for testing).
    #[must_use]
    pub fn disabled() -> Self {
        Self { secret: None }
    }

    /// Returns `true` if auth is enabled.
    #[must_use]
    pub fn is_enabled(&self) -> bool {
        self.secret.is_some()
    }

    /// Validates a bearer token against the stored secret.
    #[must_use]
    pub fn validate(&self, token: &str) -> bool {
        match &self.secret {
            None => true,
            Some(secret) => token == &**secret,
        }
    }
}

/// Paths that bypass auth even when enabled.
const PUBLIC_PATHS: &[&str] = &["/api/health"];

/// Extracts the Bearer token from the Authorization header.
fn extract_bearer_token(headers: &HeaderMap) -> Option<&str> {
    headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
}

/// Axum middleware that enforces token-based auth.
///
/// Skips auth for paths in [`PUBLIC_PATHS`] and when auth is disabled.
pub async fn auth_middleware(
    State(config): State<AuthConfig>,
    request: Request,
    next: Next,
) -> Response {
    // Auth disabled: pass through.
    if !config.is_enabled() {
        return next.run(request).await;
    }

    // Public paths: pass through.
    let path = request.uri().path();
    if PUBLIC_PATHS.contains(&path) {
        return next.run(request).await;
    }

    // Check Bearer token.
    match extract_bearer_token(request.headers()) {
        Some(token) if config.validate(token) => next.run(request).await,
        _ => (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({ "error": "unauthorized" })),
        )
            .into_response(),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    // --- AuthConfig tests ---

    #[test]
    fn disabled_config_validates_anything() {
        let config = AuthConfig::disabled();
        assert!(!config.is_enabled());
        assert!(config.validate("anything"));
        assert!(config.validate(""));
    }

    #[test]
    fn enabled_config_validates_correct_secret() {
        let config = AuthConfig::with_secret("my-secret");
        assert!(config.is_enabled());
        assert!(config.validate("my-secret"));
    }

    #[test]
    fn enabled_config_rejects_wrong_secret() {
        let config = AuthConfig::with_secret("my-secret");
        assert!(!config.validate("wrong"));
        assert!(!config.validate(""));
        assert!(!config.validate("my-secret "));
    }

    // --- Token extraction tests ---

    #[test]
    fn extract_bearer_token_valid() {
        let mut headers = HeaderMap::new();
        headers.insert("authorization", "Bearer my-token".parse().unwrap());
        assert_eq!(extract_bearer_token(&headers), Some("my-token"));
    }

    #[test]
    fn extract_bearer_token_missing_header() {
        let headers = HeaderMap::new();
        assert_eq!(extract_bearer_token(&headers), None);
    }

    #[test]
    fn extract_bearer_token_wrong_scheme() {
        let mut headers = HeaderMap::new();
        headers.insert("authorization", "Basic abc123".parse().unwrap());
        assert_eq!(extract_bearer_token(&headers), None);
    }

    #[test]
    fn extract_bearer_token_no_space_after_bearer() {
        let mut headers = HeaderMap::new();
        headers.insert("authorization", "Bearertoken".parse().unwrap());
        assert_eq!(extract_bearer_token(&headers), None);
    }

    // --- Middleware integration tests ---

    use axum::{Router, body::Body, http::Request as HttpRequest, routing::get};
    use tower::ServiceExt;

    fn test_app(config: AuthConfig) -> Router {
        Router::new()
            .route("/api/health", get(|| async { "ok" }))
            .route("/api/data", get(|| async { "secret-data" }))
            .layer(axum::middleware::from_fn_with_state(
                config.clone(),
                auth_middleware,
            ))
            .with_state(config)
    }

    #[tokio::test]
    async fn disabled_auth_allows_all() {
        let app = test_app(AuthConfig::disabled());
        let resp = app
            .oneshot(
                HttpRequest::builder()
                    .uri("/api/data")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn enabled_auth_blocks_without_token() {
        let app = test_app(AuthConfig::with_secret("s3cret"));
        let resp = app
            .oneshot(
                HttpRequest::builder()
                    .uri("/api/data")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn enabled_auth_allows_correct_token() {
        let app = test_app(AuthConfig::with_secret("s3cret"));
        let resp = app
            .oneshot(
                HttpRequest::builder()
                    .uri("/api/data")
                    .header("authorization", "Bearer s3cret")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn enabled_auth_rejects_wrong_token() {
        let app = test_app(AuthConfig::with_secret("s3cret"));
        let resp = app
            .oneshot(
                HttpRequest::builder()
                    .uri("/api/data")
                    .header("authorization", "Bearer wrong")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn health_endpoint_bypasses_auth() {
        let app = test_app(AuthConfig::with_secret("s3cret"));
        let resp = app
            .oneshot(
                HttpRequest::builder()
                    .uri("/api/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }
}
