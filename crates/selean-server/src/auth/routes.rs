//! Authentication API routes (signup, login, refresh, logout, me).

use axum::{
    Json, Router,
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::jwt::{self, JwtConfig, hash_token};
use super::middleware::CurrentUser;
use super::password;
use crate::state::AppState;

/// Creates the auth router. These routes are public (no auth required).
pub fn auth_routes() -> Router<AppState> {
    Router::new()
        .route("/api/auth/signup", post(signup))
        .route("/api/auth/login", post(login))
        .route("/api/auth/refresh", post(refresh))
        .route("/api/auth/logout", post(logout))
        .route("/api/auth/me", get(me))
}

// --- Request / Response types ---

#[derive(Debug, Deserialize)]
struct SignupRequest {
    email: String,
    password: String,
    display_name: String,
}

#[derive(Debug, Deserialize)]
struct LoginRequest {
    email: String,
    password: String,
}

#[derive(Debug, Deserialize)]
struct RefreshRequest {
    refresh_token: String,
}

#[derive(Debug, Serialize)]
struct AuthResponse {
    access_token: String,
    refresh_token: String,
    expires_in: i64,
    user: UserResponse,
}

#[derive(Debug, Serialize)]
struct UserResponse {
    id: Uuid,
    email: String,
    display_name: String,
    avatar_url: Option<String>,
}

impl From<selean_db::models::user::User> for UserResponse {
    fn from(u: selean_db::models::user::User) -> Self {
        Self {
            id: u.id,
            email: u.email,
            display_name: u.display_name,
            avatar_url: u.avatar_url,
        }
    }
}

fn error(status: StatusCode, msg: &str) -> axum::response::Response {
    (status, Json(serde_json::json!({ "error": msg }))).into_response()
}

#[allow(clippy::result_large_err)]
fn require_jwt(state: &AppState) -> Result<&JwtConfig, axum::response::Response> {
    state
        .jwt
        .as_ref()
        .ok_or_else(|| error(StatusCode::SERVICE_UNAVAILABLE, "JWT auth not configured"))
}

#[allow(clippy::result_large_err)]
fn require_db_pool(state: &AppState) -> Result<&sqlx::PgPool, axum::response::Response> {
    state
        .require_db()
        .map_err(|e| error(StatusCode::SERVICE_UNAVAILABLE, &e.to_string()))
}

/// `POST /api/auth/signup`
async fn signup(
    State(state): State<AppState>,
    Json(req): Json<SignupRequest>,
) -> axum::response::Response {
    let jwt_config = match require_jwt(&state) {
        Ok(j) => j,
        Err(resp) => return resp,
    };
    let pool = match require_db_pool(&state) {
        Ok(p) => p,
        Err(resp) => return resp,
    };

    // Validate input.
    if req.email.is_empty() || !req.email.contains('@') {
        return error(StatusCode::BAD_REQUEST, "invalid email address");
    }
    if req.password.len() < 8 {
        return error(
            StatusCode::BAD_REQUEST,
            "password must be at least 8 characters",
        );
    }
    if req.display_name.is_empty() {
        return error(StatusCode::BAD_REQUEST, "display name is required");
    }

    // Check if email already exists.
    match selean_db::queries::users::find_user_by_email(pool, &req.email).await {
        Ok(Some(_)) => return error(StatusCode::CONFLICT, "email already registered"),
        Err(e) => return error(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
        Ok(None) => {}
    }

    // Hash password.
    let password_hash = match password::hash_password(&req.password) {
        Ok(h) => h,
        Err(e) => return error(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    };

    // Create user.
    let user = match selean_db::queries::users::create_user(
        pool,
        &selean_db::models::user::CreateUser {
            email: req.email,
            display_name: req.display_name,
            password_hash: Some(password_hash),
            avatar_url: None,
        },
    )
    .await
    {
        Ok(u) => u,
        Err(e) => return error(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    };

    // Create session and tokens.
    match create_session_and_tokens(pool, jwt_config, &user).await {
        Ok(resp) => (StatusCode::CREATED, Json(resp)).into_response(),
        Err(resp) => resp,
    }
}

/// `POST /api/auth/login`
async fn login(
    State(state): State<AppState>,
    Json(req): Json<LoginRequest>,
) -> axum::response::Response {
    let jwt_config = match require_jwt(&state) {
        Ok(j) => j,
        Err(resp) => return resp,
    };
    let pool = match require_db_pool(&state) {
        Ok(p) => p,
        Err(resp) => return resp,
    };

    // Find user by email.
    let user = match selean_db::queries::users::find_user_by_email(pool, &req.email).await {
        Ok(Some(u)) => u,
        Ok(None) => return error(StatusCode::UNAUTHORIZED, "invalid email or password"),
        Err(e) => return error(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    };

    // Verify password.
    let Some(ref stored_hash) = user.password_hash else {
        return error(StatusCode::UNAUTHORIZED, "account uses OAuth login only");
    };

    if password::verify_password(&req.password, stored_hash).is_err() {
        return error(StatusCode::UNAUTHORIZED, "invalid email or password");
    }

    // Create session and tokens.
    match create_session_and_tokens(pool, jwt_config, &user).await {
        Ok(resp) => Json(resp).into_response(),
        Err(resp) => resp,
    }
}

/// `POST /api/auth/refresh`
async fn refresh(
    State(state): State<AppState>,
    Json(req): Json<RefreshRequest>,
) -> axum::response::Response {
    let jwt_config = match require_jwt(&state) {
        Ok(j) => j,
        Err(resp) => return resp,
    };
    let pool = match require_db_pool(&state) {
        Ok(p) => p,
        Err(resp) => return resp,
    };

    // Validate the refresh token JWT.
    let Ok(claims) = jwt::validate_refresh_token(jwt_config, &req.refresh_token) else {
        return error(StatusCode::UNAUTHORIZED, "invalid or expired refresh token");
    };

    // Verify session exists in DB (not revoked).
    let token_hash = hash_token(&req.refresh_token);
    if selean_db::queries::sessions::get_session_by_token(pool, &token_hash)
        .await
        .is_err()
    {
        return error(StatusCode::UNAUTHORIZED, "session revoked or expired");
    }

    // Delete old session.
    let _ = selean_db::queries::sessions::delete_session(pool, claims.sid).await;

    // Get user.
    let Ok(user) = selean_db::queries::users::get_user(pool, claims.sub).await else {
        return error(StatusCode::UNAUTHORIZED, "user not found");
    };

    // Create new session and tokens (token rotation).
    match create_session_and_tokens(pool, jwt_config, &user).await {
        Ok(resp) => Json(resp).into_response(),
        Err(resp) => resp,
    }
}

/// `POST /api/auth/logout`
async fn logout(
    State(state): State<AppState>,
    Json(req): Json<RefreshRequest>,
) -> axum::response::Response {
    let pool = match require_db_pool(&state) {
        Ok(p) => p,
        Err(resp) => return resp,
    };

    let token_hash = hash_token(&req.refresh_token);
    if let Ok(session) = selean_db::queries::sessions::get_session_by_token(pool, &token_hash).await
    {
        let _ = selean_db::queries::sessions::delete_session(pool, session.id).await;
    }

    (StatusCode::OK, Json(serde_json::json!({ "ok": true }))).into_response()
}

/// `GET /api/auth/me`
///
/// Returns the current user's profile. Requires a valid access token.
async fn me(
    State(state): State<AppState>,
    request: axum::extract::Request,
) -> axum::response::Response {
    let pool = match require_db_pool(&state) {
        Ok(p) => p,
        Err(resp) => return resp,
    };

    // Extract CurrentUser from extensions (set by auth middleware).
    let Some(current_user) = request.extensions().get::<CurrentUser>() else {
        return error(StatusCode::UNAUTHORIZED, "not authenticated");
    };

    match selean_db::queries::users::get_user(pool, current_user.id).await {
        Ok(user) => Json(UserResponse::from(user)).into_response(),
        Err(_) => error(StatusCode::NOT_FOUND, "user not found"),
    }
}

/// Helper: creates a DB session and returns token pair + user response.
async fn create_session_and_tokens(
    pool: &sqlx::PgPool,
    jwt_config: &JwtConfig,
    user: &selean_db::models::user::User,
) -> Result<AuthResponse, axum::response::Response> {
    let session_id = Uuid::new_v4();
    let token_pair = jwt::create_token_pair(jwt_config, user.id, session_id)
        .map_err(|e| error(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()))?;

    let token_hash = hash_token(&token_pair.refresh_token);
    let expires_at = chrono::Utc::now() + jwt_config.refresh_token_duration;

    selean_db::queries::sessions::create_session(pool, user.id, &token_hash, expires_at)
        .await
        .map_err(|e| error(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()))?;

    Ok(AuthResponse {
        access_token: token_pair.access_token,
        refresh_token: token_pair.refresh_token,
        expires_in: token_pair.expires_in,
        user: UserResponse {
            id: user.id,
            email: user.email.clone(),
            display_name: user.display_name.clone(),
            avatar_url: user.avatar_url.clone(),
        },
    })
}
