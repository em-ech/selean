//! GitHub OAuth and API proxy routes.
//!
//! Provides endpoints for:
//! - OAuth authorization flow (authorize URL, callback token exchange)
//! - Repository listing and creation
//! - Pushing files via the Git Data API
//! - Connection status and disconnect

use axum::{
    Json, Router,
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::auth::middleware::CurrentUser;
use crate::state::AppState;

/// GitHub API base URL.
const GITHUB_API_URL: &str = "https://api.github.com";

/// GitHub OAuth authorize URL.
const GITHUB_OAUTH_AUTHORIZE_URL: &str = "https://github.com/login/oauth/authorize";

/// GitHub OAuth token exchange URL.
const GITHUB_OAUTH_TOKEN_URL: &str = "https://github.com/login/oauth/access_token";

// ---------------------------------------------------------------------------
// Request / Response types
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct GitHubCallbackRequest {
    code: String,
    #[allow(dead_code)]
    state: String,
}

#[derive(Debug, Deserialize)]
struct CreateRepoRequest {
    name: String,
    #[serde(default)]
    private: bool,
    #[serde(default)]
    description: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PushRequest {
    /// Repository in `owner/repo` format.
    repo: String,
    /// Branch to push to (e.g. `selean/export`).
    branch: String,
    /// Files to include in the commit.
    files: Vec<PushFile>,
    /// Commit message.
    message: String,
    /// Whether to create a pull request after pushing.
    #[serde(default)]
    create_pr: bool,
}

#[derive(Debug, Deserialize)]
struct PushFile {
    path: String,
    content: String,
}

// GitHub API response types (for deserialization).

#[derive(Debug, Deserialize)]
struct GitHubUser {
    id: i64,
    login: String,
    email: Option<String>,
    name: Option<String>,
    avatar_url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GitHubTokenResponse {
    access_token: String,
    #[allow(dead_code)]
    token_type: String,
    #[allow(dead_code)]
    scope: String,
}

#[derive(Debug, Deserialize)]
struct GitHubRepo {
    full_name: String,
    default_branch: String,
    private: bool,
    permissions: Option<GitHubRepoPermissions>,
}

#[derive(Debug, Deserialize)]
struct GitHubRepoPermissions {
    push: bool,
}

#[derive(Debug, Deserialize)]
struct GitHubRef {
    object: GitHubRefObject,
}

#[derive(Debug, Deserialize)]
struct GitHubRefObject {
    sha: String,
}

#[derive(Debug, Deserialize)]
struct GitHubCommit {
    tree: GitHubTree,
}

#[derive(Debug, Deserialize)]
struct GitHubTree {
    sha: String,
}

#[derive(Debug, Deserialize)]
struct GitHubBlob {
    sha: String,
}

#[derive(Debug, Deserialize)]
struct GitHubNewTree {
    sha: String,
}

#[derive(Debug, Deserialize)]
struct GitHubNewCommit {
    sha: String,
}

#[derive(Debug, Deserialize)]
struct GitHubPullRequest {
    html_url: String,
}

// Response types sent to the client.

#[derive(Debug, Serialize)]
struct AuthorizeResponse {
    url: String,
}

#[derive(Debug, Serialize)]
struct RepoInfo {
    full_name: String,
    default_branch: String,
    private: bool,
}

#[derive(Debug, Serialize)]
struct ListReposResponse {
    repos: Vec<RepoInfo>,
}

#[derive(Debug, Serialize)]
struct PushResponse {
    success: bool,
    commit_sha: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pr_url: Option<String>,
}

#[derive(Debug, Serialize)]
struct GitHubStatusResponse {
    connected: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    login: Option<String>,
}

// ---------------------------------------------------------------------------
// Helper functions
// ---------------------------------------------------------------------------

fn error(status: StatusCode, msg: &str) -> axum::response::Response {
    (status, Json(serde_json::json!({ "error": msg }))).into_response()
}

#[allow(clippy::result_large_err)]
fn require_db(state: &AppState) -> Result<&sqlx::PgPool, axum::response::Response> {
    state
        .require_db()
        .map_err(|e| error(StatusCode::SERVICE_UNAVAILABLE, &e.to_string()))
}

#[allow(clippy::result_large_err)]
fn require_github_config(state: &AppState) -> Result<(&str, &str), axum::response::Response> {
    let client_id = state.github_client_id.as_deref().ok_or_else(|| {
        error(
            StatusCode::SERVICE_UNAVAILABLE,
            "GitHub OAuth not configured",
        )
    })?;
    let client_secret = state.github_client_secret.as_deref().ok_or_else(|| {
        error(
            StatusCode::SERVICE_UNAVAILABLE,
            "GitHub OAuth not configured",
        )
    })?;
    Ok((client_id, client_secret))
}

/// Makes a GET request to the GitHub API with authentication.
#[allow(clippy::result_large_err)]
async fn github_api_get<T: serde::de::DeserializeOwned>(
    client: &reqwest::Client,
    url: &str,
    token: &str,
) -> Result<T, axum::response::Response> {
    let resp = client
        .get(url)
        .header("Authorization", format!("Bearer {token}"))
        .header("Accept", "application/vnd.github+json")
        .header("User-Agent", "Selean")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .send()
        .await
        .map_err(|e| error(StatusCode::BAD_GATEWAY, &format!("GitHub API error: {e}")))?;

    if !resp.status().is_success() {
        let status = resp.status().as_u16();
        let body = resp
            .text()
            .await
            .unwrap_or_else(|_| String::from("(no body)"));
        return Err(error(
            map_github_status(status),
            &format!("GitHub API returned {status}: {body}"),
        ));
    }

    resp.json().await.map_err(|e| {
        error(
            StatusCode::BAD_GATEWAY,
            &format!("GitHub API parse error: {e}"),
        )
    })
}

/// Makes a POST request to the GitHub API with authentication.
#[allow(clippy::result_large_err)]
async fn github_api_post<T: serde::de::DeserializeOwned>(
    client: &reqwest::Client,
    url: &str,
    token: &str,
    body: &serde_json::Value,
) -> Result<T, axum::response::Response> {
    let resp = client
        .post(url)
        .header("Authorization", format!("Bearer {token}"))
        .header("Accept", "application/vnd.github+json")
        .header("User-Agent", "Selean")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .json(body)
        .send()
        .await
        .map_err(|e| error(StatusCode::BAD_GATEWAY, &format!("GitHub API error: {e}")))?;

    if !resp.status().is_success() {
        let status = resp.status().as_u16();
        let body = resp
            .text()
            .await
            .unwrap_or_else(|_| String::from("(no body)"));
        return Err(error(
            map_github_status(status),
            &format!("GitHub API returned {status}: {body}"),
        ));
    }

    resp.json().await.map_err(|e| {
        error(
            StatusCode::BAD_GATEWAY,
            &format!("GitHub API parse error: {e}"),
        )
    })
}

/// Makes a PATCH request to the GitHub API with authentication.
#[allow(clippy::result_large_err)]
async fn github_api_patch(
    client: &reqwest::Client,
    url: &str,
    token: &str,
    body: &serde_json::Value,
) -> Result<serde_json::Value, axum::response::Response> {
    let resp = client
        .patch(url)
        .header("Authorization", format!("Bearer {token}"))
        .header("Accept", "application/vnd.github+json")
        .header("User-Agent", "Selean")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .json(body)
        .send()
        .await
        .map_err(|e| error(StatusCode::BAD_GATEWAY, &format!("GitHub API error: {e}")))?;

    if !resp.status().is_success() {
        let status = resp.status().as_u16();
        let body = resp
            .text()
            .await
            .unwrap_or_else(|_| String::from("(no body)"));
        return Err(error(
            map_github_status(status),
            &format!("GitHub API returned {status}: {body}"),
        ));
    }

    resp.json().await.map_err(|e| {
        error(
            StatusCode::BAD_GATEWAY,
            &format!("GitHub API parse error: {e}"),
        )
    })
}

/// Maps a GitHub HTTP status to an appropriate Selean status.
fn map_github_status(status: u16) -> StatusCode {
    match status {
        401 => StatusCode::UNAUTHORIZED,
        403 => StatusCode::FORBIDDEN,
        404 => StatusCode::NOT_FOUND,
        422 => StatusCode::UNPROCESSABLE_ENTITY,
        _ => StatusCode::BAD_GATEWAY,
    }
}

/// Fetches the GitHub token for the current user from the database.
#[allow(clippy::result_large_err)]
async fn get_user_github_token(
    pool: &sqlx::PgPool,
    user_id: Uuid,
) -> Result<String, axum::response::Response> {
    let user = selean_db::queries::users::get_user(pool, user_id)
        .await
        .map_err(|e| error(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()))?;

    user.github_token.ok_or_else(|| {
        error(
            StatusCode::FORBIDDEN,
            "GitHub account not connected. Connect via /api/github/authorize first.",
        )
    })
}

// ---------------------------------------------------------------------------
// Route handlers
// ---------------------------------------------------------------------------

/// `GET /api/github/authorize`
///
/// Returns a GitHub OAuth authorization URL for the frontend to redirect to.
async fn github_authorize(State(state): State<AppState>) -> axum::response::Response {
    let (client_id, _) = match require_github_config(&state) {
        Ok(c) => c,
        Err(resp) => return resp,
    };

    // Use a random UUID as the CSRF state parameter.
    // The frontend stores this in sessionStorage and verifies it on callback.
    let csrf_state = Uuid::new_v4().to_string();

    let url =
        format!("{GITHUB_OAUTH_AUTHORIZE_URL}?client_id={client_id}&scope=repo&state={csrf_state}");

    (StatusCode::OK, Json(AuthorizeResponse { url })).into_response()
}

/// `POST /api/github/callback`
///
/// Exchanges an OAuth code for a token, fetches the GitHub user, links or
/// creates a Selean user, and returns JWT tokens.
async fn github_callback(
    State(state): State<AppState>,
    Json(body): Json<GitHubCallbackRequest>,
) -> axum::response::Response {
    let (client_id, client_secret) = match require_github_config(&state) {
        Ok(c) => c,
        Err(resp) => return resp,
    };
    let pool = match require_db(&state) {
        Ok(p) => p,
        Err(resp) => return resp,
    };
    let jwt_config = match state
        .jwt
        .as_ref()
        .ok_or_else(|| error(StatusCode::SERVICE_UNAVAILABLE, "JWT auth not configured"))
    {
        Ok(j) => j,
        Err(resp) => return resp,
    };

    if body.code.is_empty() {
        return error(StatusCode::BAD_REQUEST, "code is required");
    }

    // Exchange code for access token.
    let token_resp =
        match exchange_code_for_token(&state.http_client, client_id, client_secret, &body.code)
            .await
        {
            Ok(t) => t,
            Err(resp) => return resp,
        };

    // Fetch GitHub user profile.
    let gh_user: GitHubUser = match github_api_get(
        &state.http_client,
        &format!("{GITHUB_API_URL}/user"),
        &token_resp.access_token,
    )
    .await
    {
        Ok(u) => u,
        Err(resp) => return resp,
    };

    // Find or create the Selean user.
    let user = match find_or_create_user(pool, &gh_user, &token_resp.access_token).await {
        Ok(u) => u,
        Err(resp) => return resp,
    };

    // Issue JWT tokens.
    let session_id = Uuid::new_v4();
    let token_pair = match crate::auth::jwt::create_token_pair(jwt_config, user.id, session_id) {
        Ok(p) => p,
        Err(e) => return error(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    };

    let token_hash = crate::auth::jwt::hash_token(&token_pair.refresh_token);
    let expires_at = chrono::Utc::now() + jwt_config.refresh_token_duration;

    if let Err(e) =
        selean_db::queries::sessions::create_session(pool, user.id, &token_hash, expires_at).await
    {
        return error(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string());
    }

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "access_token": token_pair.access_token,
            "refresh_token": token_pair.refresh_token,
            "expires_in": token_pair.expires_in,
            "user": {
                "id": user.id,
                "email": user.email,
                "display_name": user.display_name,
                "avatar_url": user.avatar_url,
            }
        })),
    )
        .into_response()
}

/// Exchanges an OAuth authorization code for an access token.
#[allow(clippy::result_large_err)]
async fn exchange_code_for_token(
    client: &reqwest::Client,
    client_id: &str,
    client_secret: &str,
    code: &str,
) -> Result<GitHubTokenResponse, axum::response::Response> {
    let resp = client
        .post(GITHUB_OAUTH_TOKEN_URL)
        .header("Accept", "application/json")
        .header("User-Agent", "Selean")
        .json(&serde_json::json!({
            "client_id": client_id,
            "client_secret": client_secret,
            "code": code,
        }))
        .send()
        .await
        .map_err(|e| {
            error(
                StatusCode::BAD_GATEWAY,
                &format!("GitHub token exchange failed: {e}"),
            )
        })?;

    if !resp.status().is_success() {
        let body = resp
            .text()
            .await
            .unwrap_or_else(|_| String::from("(no body)"));
        return Err(error(
            StatusCode::BAD_GATEWAY,
            &format!("GitHub token exchange failed: {body}"),
        ));
    }

    resp.json::<GitHubTokenResponse>().await.map_err(|e| {
        error(
            StatusCode::BAD_GATEWAY,
            &format!("GitHub token parse error: {e}"),
        )
    })
}

/// Finds an existing user by `github_id` or email, or creates a new one.
/// Links the GitHub account and stores the token.
#[allow(clippy::result_large_err)]
async fn find_or_create_user(
    pool: &sqlx::PgPool,
    gh_user: &GitHubUser,
    github_token: &str,
) -> Result<selean_db::models::user::User, axum::response::Response> {
    // 1. Try to find by github_id.
    if let Some(user) = selean_db::queries::users::find_by_github_id(pool, gh_user.id)
        .await
        .map_err(|e| error(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()))?
    {
        // Update token in case it changed.
        selean_db::queries::users::update_github_token(pool, user.id, github_token)
            .await
            .map_err(|e| error(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()))?;
        return Ok(user);
    }

    // 2. Try to find by email (if GitHub provides one).
    if let Some(ref email) = gh_user.email {
        if let Some(user) = selean_db::queries::users::find_user_by_email(pool, email)
            .await
            .map_err(|e| error(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()))?
        {
            // Link GitHub to existing email account.
            selean_db::queries::users::link_github(
                pool,
                user.id,
                gh_user.id,
                &gh_user.login,
                github_token,
            )
            .await
            .map_err(|e| error(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()))?;
            return Ok(user);
        }
    }

    // 3. Create a new user.
    let display_name = gh_user
        .name
        .as_deref()
        .unwrap_or(&gh_user.login)
        .to_string();
    let email = gh_user
        .email
        .clone()
        .unwrap_or_else(|| format!("{}@users.noreply.github.com", gh_user.login));

    let user = selean_db::queries::users::create_user(
        pool,
        &selean_db::models::user::CreateUser {
            email,
            display_name: display_name.clone(),
            password_hash: None,
            avatar_url: gh_user.avatar_url.clone(),
        },
    )
    .await
    .map_err(|e| error(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()))?;

    // Link GitHub account.
    selean_db::queries::users::link_github(pool, user.id, gh_user.id, &gh_user.login, github_token)
        .await
        .map_err(|e| error(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()))?;

    // Create default workspace for new user.
    let workspace_name = format!("{display_name}'s Workspace");
    if let Err(e) =
        selean_db::queries::workspaces::create_workspace(pool, &workspace_name, user.id).await
    {
        tracing::warn!(
            "failed to create default workspace for GitHub user {}: {e}",
            user.id
        );
    }

    Ok(user)
}

/// `GET /api/github/repos`
///
/// Lists repositories where the user has push access.
async fn list_repos(
    State(state): State<AppState>,
    request: axum::extract::Request,
) -> axum::response::Response {
    let pool = match require_db(&state) {
        Ok(p) => p,
        Err(resp) => return resp,
    };
    let Some(current_user) = request.extensions().get::<CurrentUser>() else {
        return error(StatusCode::UNAUTHORIZED, "not authenticated");
    };

    let token = match get_user_github_token(pool, current_user.id).await {
        Ok(t) => t,
        Err(resp) => return resp,
    };

    let url = format!(
        "{GITHUB_API_URL}/user/repos?sort=updated&per_page=30&affiliation=owner,collaborator"
    );
    let repos: Vec<GitHubRepo> = match github_api_get(&state.http_client, &url, &token).await {
        Ok(r) => r,
        Err(resp) => return resp,
    };

    let repos: Vec<RepoInfo> = repos
        .into_iter()
        .filter(|r| r.permissions.as_ref().is_some_and(|p| p.push))
        .map(|r| RepoInfo {
            full_name: r.full_name,
            default_branch: r.default_branch,
            private: r.private,
        })
        .collect();

    (StatusCode::OK, Json(ListReposResponse { repos })).into_response()
}

/// `POST /api/github/repos`
///
/// Creates a new GitHub repository.
async fn create_repo(
    State(state): State<AppState>,
    request: axum::extract::Request,
) -> axum::response::Response {
    let pool = match require_db(&state) {
        Ok(p) => p,
        Err(resp) => return resp,
    };

    let (current_user, body_bytes) = match extract_user_and_body(request).await {
        Ok(v) => v,
        Err(resp) => return resp,
    };

    let token = match get_user_github_token(pool, current_user.id).await {
        Ok(t) => t,
        Err(resp) => return resp,
    };

    let req_body: CreateRepoRequest = match serde_json::from_slice(&body_bytes) {
        Ok(b) => b,
        Err(e) => return error(StatusCode::BAD_REQUEST, &format!("invalid request: {e}")),
    };

    if req_body.name.is_empty() {
        return error(StatusCode::BAD_REQUEST, "name is required");
    }

    let create_body = serde_json::json!({
        "name": req_body.name,
        "private": req_body.private,
        "description": req_body.description,
        "auto_init": true,
    });

    let repo: GitHubRepo = match github_api_post(
        &state.http_client,
        &format!("{GITHUB_API_URL}/user/repos"),
        &token,
        &create_body,
    )
    .await
    {
        Ok(r) => r,
        Err(resp) => return resp,
    };

    (
        StatusCode::CREATED,
        Json(RepoInfo {
            full_name: repo.full_name,
            default_branch: repo.default_branch,
            private: repo.private,
        }),
    )
        .into_response()
}

/// `POST /api/github/push`
///
/// Pushes files to a repository using the Git Data API.
async fn push_to_repo(
    State(state): State<AppState>,
    request: axum::extract::Request,
) -> axum::response::Response {
    let pool = match require_db(&state) {
        Ok(p) => p,
        Err(resp) => return resp,
    };

    let (current_user, body_bytes) = match extract_user_and_body(request).await {
        Ok(v) => v,
        Err(resp) => return resp,
    };

    let token = match get_user_github_token(pool, current_user.id).await {
        Ok(t) => t,
        Err(resp) => return resp,
    };

    let req_body: PushRequest = match serde_json::from_slice(&body_bytes) {
        Ok(b) => b,
        Err(e) => return error(StatusCode::BAD_REQUEST, &format!("invalid request: {e}")),
    };

    if let Err(resp) = validate_push_request(&req_body) {
        return resp;
    }

    match execute_push(&state.http_client, &token, &req_body).await {
        Ok(resp) => (StatusCode::OK, Json(resp)).into_response(),
        Err(resp) => resp,
    }
}

/// Validates the push request fields.
#[allow(clippy::result_large_err)]
fn validate_push_request(req: &PushRequest) -> Result<(), axum::response::Response> {
    if req.repo.is_empty() || !req.repo.contains('/') {
        return Err(error(
            StatusCode::BAD_REQUEST,
            "repo must be in owner/repo format",
        ));
    }
    if req.branch.is_empty() {
        return Err(error(StatusCode::BAD_REQUEST, "branch is required"));
    }
    if req.files.is_empty() {
        return Err(error(
            StatusCode::BAD_REQUEST,
            "at least one file is required",
        ));
    }
    if req.message.is_empty() {
        return Err(error(StatusCode::BAD_REQUEST, "message is required"));
    }
    Ok(())
}

/// Executes the Git Data API push workflow.
#[allow(clippy::result_large_err)]
async fn execute_push(
    client: &reqwest::Client,
    token: &str,
    req: &PushRequest,
) -> Result<PushResponse, axum::response::Response> {
    let repo_url = format!("{GITHUB_API_URL}/repos/{}", req.repo);

    // 1. Get the default branch ref to use as base.
    let default_ref: GitHubRef =
        match github_api_get(client, &format!("{repo_url}/git/ref/heads/main"), token).await {
            Ok(r) => r,
            // Try "master" as fallback.
            Err(_) => {
                github_api_get(client, &format!("{repo_url}/git/ref/heads/master"), token).await?
            }
        };

    let base_sha = &default_ref.object.sha;

    // 2. Get or create the target branch.
    let branch_sha = get_or_create_branch(client, token, &repo_url, &req.branch, base_sha).await?;

    // 3. Get the tree SHA of the branch head commit.
    let commit: GitHubCommit = github_api_get(
        client,
        &format!("{repo_url}/git/commits/{branch_sha}"),
        token,
    )
    .await?;
    let base_tree_sha = &commit.tree.sha;

    // 4. Create blobs for each file and build tree entries.
    let mut tree_entries = Vec::with_capacity(req.files.len());
    for file in &req.files {
        let blob: GitHubBlob = github_api_post(
            client,
            &format!("{repo_url}/git/blobs"),
            token,
            &serde_json::json!({
                "content": file.content,
                "encoding": "utf-8",
            }),
        )
        .await?;

        tree_entries.push(serde_json::json!({
            "path": file.path,
            "mode": "100644",
            "type": "blob",
            "sha": blob.sha,
        }));
    }

    // 5. Create a new tree.
    let new_tree: GitHubNewTree = github_api_post(
        client,
        &format!("{repo_url}/git/trees"),
        token,
        &serde_json::json!({
            "base_tree": base_tree_sha,
            "tree": tree_entries,
        }),
    )
    .await?;

    // 6. Create a new commit.
    let new_commit: GitHubNewCommit = github_api_post(
        client,
        &format!("{repo_url}/git/commits"),
        token,
        &serde_json::json!({
            "message": req.message,
            "tree": new_tree.sha,
            "parents": [branch_sha],
        }),
    )
    .await?;

    // 7. Update the branch ref to point to the new commit.
    github_api_patch(
        client,
        &format!("{repo_url}/git/refs/heads/{}", req.branch),
        token,
        &serde_json::json!({ "sha": new_commit.sha }),
    )
    .await?;

    // 8. Optionally create a pull request.
    let pr_url = if req.create_pr {
        let pr: GitHubPullRequest = github_api_post(
            client,
            &format!("{repo_url}/pulls"),
            token,
            &serde_json::json!({
                "title": req.message,
                "head": req.branch,
                "base": "main",
            }),
        )
        .await?;
        Some(pr.html_url)
    } else {
        None
    };

    Ok(PushResponse {
        success: true,
        commit_sha: new_commit.sha,
        pr_url,
    })
}

/// Gets an existing branch ref, or creates it from a base SHA.
#[allow(clippy::result_large_err)]
async fn get_or_create_branch(
    client: &reqwest::Client,
    token: &str,
    repo_url: &str,
    branch: &str,
    base_sha: &str,
) -> Result<String, axum::response::Response> {
    let ref_url = format!("{repo_url}/git/ref/heads/{branch}");

    // Try to get the existing branch.
    let resp = client
        .get(&ref_url)
        .header("Authorization", format!("Bearer {token}"))
        .header("Accept", "application/vnd.github+json")
        .header("User-Agent", "Selean")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .send()
        .await
        .map_err(|e| error(StatusCode::BAD_GATEWAY, &format!("GitHub API error: {e}")))?;

    if resp.status().is_success() {
        let git_ref: GitHubRef = resp.json().await.map_err(|e| {
            error(
                StatusCode::BAD_GATEWAY,
                &format!("GitHub API parse error: {e}"),
            )
        })?;
        return Ok(git_ref.object.sha);
    }

    // Branch doesn't exist: create it.
    let new_ref: GitHubRef = github_api_post(
        client,
        &format!("{repo_url}/git/refs"),
        token,
        &serde_json::json!({
            "ref": format!("refs/heads/{branch}"),
            "sha": base_sha,
        }),
    )
    .await?;

    Ok(new_ref.object.sha)
}

/// `GET /api/github/status`
///
/// Returns whether the current user has GitHub connected.
async fn github_status(
    State(state): State<AppState>,
    request: axum::extract::Request,
) -> axum::response::Response {
    let pool = match require_db(&state) {
        Ok(p) => p,
        Err(resp) => return resp,
    };
    let Some(current_user) = request.extensions().get::<CurrentUser>() else {
        return error(StatusCode::UNAUTHORIZED, "not authenticated");
    };

    let user = match selean_db::queries::users::get_user(pool, current_user.id).await {
        Ok(u) => u,
        Err(e) => return error(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    };

    let connected = user.github_token.is_some();
    let login = if connected { user.github_login } else { None };

    (
        StatusCode::OK,
        Json(GitHubStatusResponse { connected, login }),
    )
        .into_response()
}

/// `POST /api/github/disconnect`
///
/// Removes the GitHub connection from the current user.
async fn disconnect_github(
    State(state): State<AppState>,
    request: axum::extract::Request,
) -> axum::response::Response {
    let pool = match require_db(&state) {
        Ok(p) => p,
        Err(resp) => return resp,
    };
    let Some(current_user) = request.extensions().get::<CurrentUser>() else {
        return error(StatusCode::UNAUTHORIZED, "not authenticated");
    };

    match selean_db::queries::users::disconnect_github(pool, current_user.id).await {
        Ok(()) => (StatusCode::OK, Json(serde_json::json!({ "ok": true }))).into_response(),
        Err(e) => error(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

/// Extracts `CurrentUser` from extensions and reads the body bytes.
#[allow(clippy::result_large_err)]
async fn extract_user_and_body(
    request: axum::extract::Request,
) -> Result<(CurrentUser, axum::body::Bytes), axum::response::Response> {
    let current_user = request
        .extensions()
        .get::<CurrentUser>()
        .cloned()
        .ok_or_else(|| error(StatusCode::UNAUTHORIZED, "not authenticated"))?;

    let body = axum::body::to_bytes(request.into_body(), 1024 * 1024)
        .await
        .map_err(|e| {
            error(
                StatusCode::BAD_REQUEST,
                &format!("failed to read body: {e}"),
            )
        })?;

    Ok((current_user, body))
}

// ---------------------------------------------------------------------------
// Router
// ---------------------------------------------------------------------------

/// Creates the GitHub routes router.
///
/// The `/api/github/authorize` and `/api/github/callback` routes are public
/// (accessed before the user is authenticated). All other routes require auth.
pub fn github_routes() -> Router<AppState> {
    Router::new()
        .route("/api/github/authorize", get(github_authorize))
        .route("/api/github/callback", post(github_callback))
        .route("/api/github/repos", get(list_repos).post(create_repo))
        .route("/api/github/push", post(push_to_repo))
        .route("/api/github/status", get(github_status))
        .route("/api/github/disconnect", post(disconnect_github))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn authorize_response_serializes() {
        let resp = AuthorizeResponse {
            url: "https://github.com/login/oauth/authorize?client_id=abc".to_string(),
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("github.com"));
    }

    #[test]
    fn list_repos_response_serializes() {
        let resp = ListReposResponse {
            repos: vec![RepoInfo {
                full_name: "user/repo".to_string(),
                default_branch: "main".to_string(),
                private: false,
            }],
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("user/repo"));
        assert!(json.contains("main"));
    }

    #[test]
    fn push_response_serializes_without_pr_url() {
        let resp = PushResponse {
            success: true,
            commit_sha: "abc123".to_string(),
            pr_url: None,
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("abc123"));
        assert!(!json.contains("pr_url"));
    }

    #[test]
    fn push_response_serializes_with_pr_url() {
        let resp = PushResponse {
            success: true,
            commit_sha: "abc123".to_string(),
            pr_url: Some("https://github.com/user/repo/pull/1".to_string()),
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("pr_url"));
        assert!(json.contains("pull/1"));
    }

    #[test]
    fn github_status_response_serializes_connected() {
        let resp = GitHubStatusResponse {
            connected: true,
            login: Some("octocat".to_string()),
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("true"));
        assert!(json.contains("octocat"));
    }

    #[test]
    fn github_status_response_serializes_disconnected() {
        let resp = GitHubStatusResponse {
            connected: false,
            login: None,
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("false"));
        // login should be omitted when None.
        assert!(!json.contains("login"));
    }

    #[test]
    fn callback_request_deserializes() {
        let json = r#"{"code":"abc123","state":"some-state"}"#;
        let req: GitHubCallbackRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.code, "abc123");
        assert_eq!(req.state, "some-state");
    }

    #[test]
    fn create_repo_request_deserializes_minimal() {
        let json = r#"{"name":"my-repo"}"#;
        let req: CreateRepoRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.name, "my-repo");
        assert!(!req.private);
        assert!(req.description.is_none());
    }

    #[test]
    fn create_repo_request_deserializes_full() {
        let json = r#"{"name":"my-repo","private":true,"description":"A test repo"}"#;
        let req: CreateRepoRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.name, "my-repo");
        assert!(req.private);
        assert_eq!(req.description.as_deref(), Some("A test repo"));
    }

    #[test]
    fn push_request_deserializes() {
        let json = r#"{
            "repo": "user/repo",
            "branch": "selean/export",
            "files": [{"path": "index.html", "content": "<html/>"}],
            "message": "Export from Selean",
            "create_pr": true
        }"#;
        let req: PushRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.repo, "user/repo");
        assert_eq!(req.branch, "selean/export");
        assert_eq!(req.files.len(), 1);
        assert_eq!(req.files[0].path, "index.html");
        assert!(req.create_pr);
    }

    #[test]
    fn push_request_defaults_create_pr_to_false() {
        let json = r#"{
            "repo": "user/repo",
            "branch": "main",
            "files": [{"path": "a.txt", "content": "hello"}],
            "message": "commit"
        }"#;
        let req: PushRequest = serde_json::from_str(json).unwrap();
        assert!(!req.create_pr);
    }

    #[test]
    fn github_user_deserializes() {
        let json = r#"{
            "id": 12345,
            "login": "octocat",
            "email": "octocat@github.com",
            "name": "The Octocat",
            "avatar_url": "https://avatars.githubusercontent.com/u/12345"
        }"#;
        let user: GitHubUser = serde_json::from_str(json).unwrap();
        assert_eq!(user.id, 12345);
        assert_eq!(user.login, "octocat");
        assert_eq!(user.email.as_deref(), Some("octocat@github.com"));
    }

    #[test]
    fn github_user_deserializes_without_optional_fields() {
        let json = r#"{
            "id": 99,
            "login": "ghost",
            "email": null,
            "name": null,
            "avatar_url": null
        }"#;
        let user: GitHubUser = serde_json::from_str(json).unwrap();
        assert_eq!(user.id, 99);
        assert!(user.email.is_none());
        assert!(user.name.is_none());
    }

    #[test]
    fn github_token_response_deserializes() {
        let json = r#"{"access_token":"gho_abc123","token_type":"bearer","scope":"repo"}"#;
        let resp: GitHubTokenResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.access_token, "gho_abc123");
    }

    #[test]
    fn github_repo_deserializes() {
        let json = r#"{
            "full_name": "octocat/hello-world",
            "default_branch": "main",
            "private": false,
            "permissions": {"push": true}
        }"#;
        let repo: GitHubRepo = serde_json::from_str(json).unwrap();
        assert_eq!(repo.full_name, "octocat/hello-world");
        assert!(repo.permissions.unwrap().push);
    }

    #[test]
    fn validate_push_request_empty_repo() {
        let req = PushRequest {
            repo: String::new(),
            branch: "main".to_string(),
            files: vec![PushFile {
                path: "a.txt".to_string(),
                content: "hello".to_string(),
            }],
            message: "msg".to_string(),
            create_pr: false,
        };
        assert!(validate_push_request(&req).is_err());
    }

    #[test]
    fn validate_push_request_no_slash_in_repo() {
        let req = PushRequest {
            repo: "noslash".to_string(),
            branch: "main".to_string(),
            files: vec![PushFile {
                path: "a.txt".to_string(),
                content: "hello".to_string(),
            }],
            message: "msg".to_string(),
            create_pr: false,
        };
        assert!(validate_push_request(&req).is_err());
    }

    #[test]
    fn validate_push_request_empty_branch() {
        let req = PushRequest {
            repo: "user/repo".to_string(),
            branch: String::new(),
            files: vec![PushFile {
                path: "a.txt".to_string(),
                content: "hello".to_string(),
            }],
            message: "msg".to_string(),
            create_pr: false,
        };
        assert!(validate_push_request(&req).is_err());
    }

    #[test]
    fn validate_push_request_no_files() {
        let req = PushRequest {
            repo: "user/repo".to_string(),
            branch: "main".to_string(),
            files: vec![],
            message: "msg".to_string(),
            create_pr: false,
        };
        assert!(validate_push_request(&req).is_err());
    }

    #[test]
    fn validate_push_request_empty_message() {
        let req = PushRequest {
            repo: "user/repo".to_string(),
            branch: "main".to_string(),
            files: vec![PushFile {
                path: "a.txt".to_string(),
                content: "hello".to_string(),
            }],
            message: String::new(),
            create_pr: false,
        };
        assert!(validate_push_request(&req).is_err());
    }

    #[test]
    fn validate_push_request_valid() {
        let req = PushRequest {
            repo: "user/repo".to_string(),
            branch: "selean/export".to_string(),
            files: vec![PushFile {
                path: "a.txt".to_string(),
                content: "hello".to_string(),
            }],
            message: "Export from Selean".to_string(),
            create_pr: false,
        };
        assert!(validate_push_request(&req).is_ok());
    }

    #[test]
    fn map_github_status_codes() {
        assert_eq!(map_github_status(401), StatusCode::UNAUTHORIZED);
        assert_eq!(map_github_status(403), StatusCode::FORBIDDEN);
        assert_eq!(map_github_status(404), StatusCode::NOT_FOUND);
        assert_eq!(map_github_status(422), StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(map_github_status(500), StatusCode::BAD_GATEWAY);
        assert_eq!(map_github_status(503), StatusCode::BAD_GATEWAY);
    }
}
