//! Auth middleware supporting both legacy shared secret and JWT modes.

use axum::{
    Json,
    extract::{Request, State},
    http::{HeaderMap, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use uuid::Uuid;

use super::config::AuthConfig;
use super::jwt::{JwtConfig, validate_access_token};

/// Authenticated user identity, inserted as a request extension.
#[derive(Clone, Debug)]
pub struct CurrentUser {
    /// The authenticated user's ID.
    pub id: Uuid,
}

/// Combined auth state for the middleware.
#[derive(Clone)]
pub struct AuthState {
    /// Legacy shared secret config.
    pub config: AuthConfig,
    /// JWT config (when `JWT_SECRET` is set).
    pub jwt: Option<JwtConfig>,
}

/// Paths that bypass auth even when enabled.
const PUBLIC_PATHS: &[&str] = &["/api/health", "/api/health/ready"];

/// Path prefix for auth routes that must be public.
const AUTH_PATH_PREFIX: &str = "/api/auth/";

/// Extracts the Bearer token from the Authorization header.
fn extract_bearer_token(headers: &HeaderMap) -> Option<&str> {
    headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
}

/// Axum middleware that enforces authentication.
///
/// Auth flow:
/// 1. If auth is fully disabled (no secret, no JWT), pass through.
/// 2. If the path is public, pass through.
/// 3. If JWT is configured, try to validate as JWT first.
/// 4. Fall back to legacy shared secret validation.
pub async fn auth_middleware(
    State(state): State<AuthState>,
    mut request: Request,
    next: Next,
) -> Response {
    // Auth fully disabled: pass through.
    if !state.config.is_enabled() && state.jwt.is_none() {
        return next.run(request).await;
    }

    // Public paths: pass through.
    let path = request.uri().path();
    if PUBLIC_PATHS.contains(&path) || path.starts_with(AUTH_PATH_PREFIX) {
        return next.run(request).await;
    }

    // Extract bearer token.
    let Some(token) = extract_bearer_token(request.headers()) else {
        return unauthorized();
    };

    // Try JWT validation first (if configured).
    if let Some(ref jwt_config) = state.jwt {
        match validate_access_token(jwt_config, token) {
            Ok(claims) => {
                request
                    .extensions_mut()
                    .insert(CurrentUser { id: claims.sub });
                return next.run(request).await;
            }
            // If JWT validation fails, don't fall through to legacy -- reject.
            Err(_) if state.config.is_enabled() => {
                // If legacy secret is also configured, try it.
                if state.config.validate(token) {
                    return next.run(request).await;
                }
                return unauthorized();
            }
            Err(_) => return unauthorized(),
        }
    }

    // Legacy shared secret only.
    if state.config.validate(token) {
        return next.run(request).await;
    }

    unauthorized()
}

fn unauthorized() -> Response {
    (
        StatusCode::UNAUTHORIZED,
        Json(serde_json::json!({ "error": "unauthorized" })),
    )
        .into_response()
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::auth::jwt::{JwtConfig, create_access_token};

    use axum::{Router, body::Body, http::Request as HttpRequest, routing::get};
    use tower::ServiceExt;

    fn make_app(config: AuthConfig, jwt: Option<JwtConfig>) -> Router {
        let state = AuthState { config, jwt };
        Router::new()
            .route("/api/health", get(|| async { "ok" }))
            .route("/api/auth/login", get(|| async { "login" }))
            .route("/api/data", get(|| async { "secret" }))
            .layer(axum::middleware::from_fn_with_state(
                state.clone(),
                auth_middleware,
            ))
            .with_state(state)
    }

    #[tokio::test]
    async fn disabled_allows_all() {
        let app = make_app(AuthConfig::disabled(), None);
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
    async fn health_bypasses_auth() {
        let app = make_app(AuthConfig::with_secret("s3cret"), None);
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

    #[tokio::test]
    async fn auth_routes_bypass_auth() {
        let app = make_app(AuthConfig::with_secret("s3cret"), None);
        let resp = app
            .oneshot(
                HttpRequest::builder()
                    .uri("/api/auth/login")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn legacy_secret_works() {
        let app = make_app(AuthConfig::with_secret("s3cret"), None);
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
    async fn jwt_token_works() {
        let jwt = JwtConfig::new("test-secret");
        let user_id = Uuid::new_v4();
        let token = create_access_token(&jwt, user_id).expect("create token");

        let app = make_app(AuthConfig::disabled(), Some(jwt));
        let resp = app
            .oneshot(
                HttpRequest::builder()
                    .uri("/api/data")
                    .header("authorization", format!("Bearer {token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn invalid_jwt_rejected() {
        let jwt = JwtConfig::new("test-secret");
        let app = make_app(AuthConfig::disabled(), Some(jwt));
        let resp = app
            .oneshot(
                HttpRequest::builder()
                    .uri("/api/data")
                    .header("authorization", "Bearer invalid.jwt.token")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn no_token_rejected_when_jwt_enabled() {
        let jwt = JwtConfig::new("test-secret");
        let app = make_app(AuthConfig::disabled(), Some(jwt));
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
}
