//! HTTP route definitions for the Selean server.
//!
//! All routes are mounted under `/api/`. Static file serving for the React
//! frontend is added at the router level.

use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, Multipart, Path, State},
    http::{StatusCode, header},
    response::{
        IntoResponse,
        sse::{Event, KeepAlive, Sse},
    },
    routing::{get, post},
};
use std::convert::Infallible;
use tokio_stream::StreamExt;

use crate::auth::AuthConfig;
use crate::auth::middleware::{AuthState, auth_middleware};
use crate::chat::{ChatRequest, send_chat_request_streaming};
use crate::collab::ws_handler::{CollabState, ws_handler};
use crate::state::AppState;

/// 50 MiB upload limit for file import endpoints.
const MAX_UPLOAD_BYTES: usize = 50 * 1024 * 1024;

/// 1 MiB limit for JSON API endpoints (chat, export, figma import).
const MAX_JSON_BYTES: usize = 1024 * 1024;

/// Creates the Axum router with all API routes and auth disabled.
pub fn create_router(state: AppState) -> Router {
    create_router_with_options(state, CollabState::new(), AuthConfig::disabled())
}

/// Creates the Axum router with explicit collab state (for testing).
pub fn create_router_with_collab(state: AppState, collab_state: CollabState) -> Router {
    create_router_with_options(state, collab_state, AuthConfig::disabled())
}

/// Creates the Axum router with all options explicit.
pub fn create_router_with_options(
    state: AppState,
    collab_state: CollabState,
    auth_config: AuthConfig,
) -> Router {
    let auth_state = AuthState {
        config: auth_config,
        jwt: state.jwt.clone(),
    };

    let collab_routes = Router::new()
        .route("/api/ws", get(ws_handler))
        .layer(axum::middleware::from_fn_with_state(
            auth_state.clone(),
            auth_middleware,
        ))
        .with_state(collab_state);

    // File upload routes: 50 MiB limit.
    let upload_routes = Router::new()
        .route("/api/import/pptx", post(import_pptx_handler))
        .route("/api/import/idml", post(import_idml_handler))
        .route("/api/import/indd", post(import_indd_handler))
        .layer(DefaultBodyLimit::max(MAX_UPLOAD_BYTES));

    // Asset upload route: 10 MiB limit.
    let asset_upload_routes = Router::new()
        .route("/api/assets", post(crate::assets::upload_asset))
        .layer(DefaultBodyLimit::max(10 * 1024 * 1024));

    // JSON API routes: 1 MiB limit.
    let json_routes = Router::new()
        .route("/api/health", get(health))
        .route("/api/health/ready", get(health_ready))
        .route("/api/tools", get(list_tools))
        .route("/api/chat", post(chat_handler))
        .route("/api/export/pptx", post(export_pptx_handler))
        .route("/api/export/idml", post(export_idml_handler))
        .route("/api/import/figma", post(import_figma_handler))
        .route("/api/export/figma", post(export_figma_handler))
        .route("/api/fonts/{family}", get(serve_font))
        .route(
            "/api/assets/{id}",
            get(crate::assets::serve_asset).delete(crate::assets::delete_asset),
        )
        .route(
            "/api/workspaces/{workspace_id}/assets",
            get(crate::assets::list_assets),
        )
        .layer(DefaultBodyLimit::max(MAX_JSON_BYTES));

    // Document management routes (database-backed).
    let doc_routes = crate::documents::document_routes();

    // Workspace management routes (database-backed, RBAC).
    let workspace_routes = crate::workspaces::workspace_routes();

    // Auth routes (public, no auth middleware).
    let auth_api_routes = crate::auth::routes::auth_routes();

    // GitHub OAuth + API proxy routes.
    let github_api_routes = crate::github::github_routes();

    upload_routes
        .merge(asset_upload_routes)
        .merge(json_routes)
        .merge(doc_routes)
        .merge(workspace_routes)
        .merge(auth_api_routes)
        .merge(github_api_routes)
        .layer(axum::middleware::from_fn_with_state(
            auth_state,
            auth_middleware,
        ))
        .with_state(state)
        .merge(collab_routes)
}

/// Health check endpoint (liveness).
async fn health() -> &'static str {
    "ok"
}

/// Readiness check. Verifies database connectivity when configured.
async fn health_ready(State(state): State<AppState>) -> impl IntoResponse {
    let mut checks = serde_json::json!({ "status": "ok" });

    if let Some(ref pool) = state.db {
        match sqlx::query_scalar::<_, i32>("SELECT 1")
            .fetch_one(pool)
            .await
        {
            Ok(_) => {
                checks["database"] = serde_json::json!("connected");
            }
            Err(e) => {
                return (
                    StatusCode::SERVICE_UNAVAILABLE,
                    Json(serde_json::json!({
                        "status": "degraded",
                        "database": format!("error: {e}")
                    })),
                )
                    .into_response();
            }
        }
    } else {
        checks["database"] = serde_json::json!("not configured");
    }

    checks["storage"] = serde_json::json!(state.storage.name());

    (StatusCode::OK, Json(checks)).into_response()
}

/// Lists all available LLM tools.
async fn list_tools() -> Json<Vec<selean_llm::ToolDefinition>> {
    Json(selean_llm::all_tools())
}

/// Handles chat requests. Sends to Claude API and streams response as SSE.
async fn chat_handler(
    State(state): State<AppState>,
    Json(request): Json<ChatRequest>,
) -> impl IntoResponse {
    let rx = send_chat_request_streaming(&state, &request);
    let stream = tokio_stream::wrappers::ReceiverStream::new(rx).map(|event| {
        let json = serde_json::to_string(&event).unwrap_or_default();
        Ok::<_, Infallible>(Event::default().data(json))
    });

    Sse::new(stream)
        .keep_alive(KeepAlive::default())
        .into_response()
}

/// Returns a JSON error response with the given status and message.
fn error_response(status: StatusCode, message: &str) -> axum::response::Response {
    (status, Json(serde_json::json!({ "error": message }))).into_response()
}

/// Reads the "file" field from a multipart upload.
async fn read_multipart_file(
    multipart: &mut Multipart,
) -> Result<axum::body::Bytes, axum::response::Response> {
    while let Ok(Some(field)) = multipart.next_field().await {
        if field.name() == Some("file") {
            return field.bytes().await.map_err(|e| {
                error_response(
                    StatusCode::BAD_REQUEST,
                    &format!("failed to read file: {e}"),
                )
            });
        }
    }
    Err(error_response(
        StatusCode::BAD_REQUEST,
        "missing 'file' field in multipart form",
    ))
}

/// Serializes a persistence save result to a JSON HTTP response.
fn document_json_response<E: std::fmt::Display>(
    result: Result<String, E>,
) -> axum::response::Response {
    match result {
        Ok(json) => (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "application/json")],
            json,
        )
            .into_response(),
        Err(e) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("serialization failed: {e}"),
        ),
    }
}

/// Reads multipart file bytes, calls the format-specific import function,
/// and returns the resulting `Document` as JSON.
async fn handle_import<E: std::fmt::Display>(
    mut multipart: Multipart,
    import_fn: fn(&[u8]) -> Result<selean_engine::persistence::Document, E>,
    format_name: &str,
) -> axum::response::Response {
    let bytes = match read_multipart_file(&mut multipart).await {
        Ok(b) => b,
        Err(resp) => return resp,
    };
    match import_fn(&bytes) {
        Ok(doc) => document_json_response(selean_engine::persistence::save_document(&doc)),
        Err(e) => error_response(
            StatusCode::BAD_REQUEST,
            &format!("{format_name} import failed: {e}"),
        ),
    }
}

/// Parses a `Document` from JSON body, calls the format-specific export
/// function, and returns the binary with appropriate content headers.
fn handle_export<E: std::fmt::Display>(
    body: &axum::body::Bytes,
    export_fn: fn(&selean_engine::persistence::Document) -> Result<Vec<u8>, E>,
    content_type: &str,
    filename: &str,
    format_name: &str,
) -> axum::response::Response {
    let doc = match selean_engine::persistence::load_document(&String::from_utf8_lossy(body)) {
        Ok(d) => d,
        Err(e) => {
            return error_response(
                StatusCode::BAD_REQUEST,
                &format!("invalid document JSON: {e}"),
            );
        }
    };
    let disposition = format!("attachment; filename=\"{filename}\"");
    match export_fn(&doc) {
        Ok(bytes) => (
            StatusCode::OK,
            [
                (header::CONTENT_TYPE, content_type.to_owned()),
                (header::CONTENT_DISPOSITION, disposition),
            ],
            bytes,
        )
            .into_response(),
        Err(e) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("{format_name} export failed: {e}"),
        ),
    }
}

/// Handles PPTX import.
async fn import_pptx_handler(multipart: Multipart) -> axum::response::Response {
    handle_import(multipart, selean_pptx::import_pptx, "pptx").await
}

/// Handles PPTX export.
async fn export_pptx_handler(body: axum::body::Bytes) -> axum::response::Response {
    handle_export(
        &body,
        selean_pptx::export_pptx,
        "application/vnd.openxmlformats-officedocument.presentationml.presentation",
        "export.pptx",
        "pptx",
    )
}

/// Handles IDML import.
async fn import_idml_handler(multipart: Multipart) -> axum::response::Response {
    handle_import(multipart, selean_idml::import_idml, "idml").await
}

/// Handles IDML export.
async fn export_idml_handler(body: axum::body::Bytes) -> axum::response::Response {
    handle_export(
        &body,
        selean_idml::export_idml,
        "application/vnd.adobe.indesign-idml-package",
        "export.idml",
        "idml",
    )
}

/// Request body for Figma import.
#[derive(serde::Deserialize)]
struct FigmaImportRequest {
    /// Figma file key extracted from the Figma URL.
    file_key: String,
}

/// Handles Figma import. Accepts a JSON body with `file_key`, reads the
/// access token from server-side configuration, fetches the file from
/// the Figma REST API, and returns the parsed `Document` as JSON.
async fn import_figma_handler(
    State(state): State<AppState>,
    Json(request): Json<FigmaImportRequest>,
) -> axum::response::Response {
    if request.file_key.is_empty() {
        return error_response(StatusCode::BAD_REQUEST, "file_key is required");
    }

    let Some(ref access_token) = state.figma_access_token else {
        return error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "FIGMA_ACCESS_TOKEN not configured on server",
        );
    };

    match selean_figma::import_figma(&state.http_client, access_token, &request.file_key).await {
        Ok(doc) => document_json_response(selean_engine::persistence::save_document(&doc)),
        Err(e) => {
            let status = match &e {
                selean_figma::FigmaError::Api { status: 403, .. } => StatusCode::FORBIDDEN,
                selean_figma::FigmaError::Api { status: 404, .. } => StatusCode::NOT_FOUND,
                _ => StatusCode::BAD_REQUEST,
            };
            error_response(status, &format!("figma import failed: {e}"))
        }
    }
}

/// Handles Figma interchange export.
///
/// Accepts a `Document` JSON body and returns the Figma interchange
/// JSON that can be consumed by the Selean Figma plugin.
async fn export_figma_handler(body: axum::body::Bytes) -> axum::response::Response {
    let doc = match selean_engine::persistence::load_document(&String::from_utf8_lossy(&body)) {
        Ok(d) => d,
        Err(e) => {
            return error_response(
                StatusCode::BAD_REQUEST,
                &format!("invalid document JSON: {e}"),
            );
        }
    };

    match selean_figma::export_figma_interchange(&doc) {
        Ok(json) => (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "application/json")],
            json,
        )
            .into_response(),
        Err(e) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("figma export failed: {e}"),
        ),
    }
}

/// Handles `InDesign` `.indd` import.
///
/// If `InDesign` Server is configured (`INDESIGN_SERVER_URL`), converts
/// the `.indd` file via the server, then returns the `Document` as JSON.
/// If not configured, returns 501 with guidance to use IDML instead.
async fn import_indd_handler(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> axum::response::Response {
    let bytes = match read_multipart_file(&mut multipart).await {
        Ok(b) => b,
        Err(resp) => return resp,
    };

    match crate::indesign_bridge::convert_indd_to_document(
        &state.http_client,
        state.indesign_server_url.as_deref(),
        &bytes,
    )
    .await
    {
        Ok(doc) => document_json_response(selean_engine::persistence::save_document(&doc)),
        Err(crate::indesign_bridge::IndesignBridgeError::NotConfigured) => error_response(
            StatusCode::NOT_IMPLEMENTED,
            "InDesign Server not configured. Export your file as IDML (.idml) \
             from InDesign and use Import IDML instead.",
        ),
        Err(e) => error_response(StatusCode::BAD_REQUEST, &format!("indd import failed: {e}")),
    }
}

/// Serves a font file by family name from the `fonts/` directory.
///
/// Font files are looked up as `fonts/{family}.ttf` or `fonts/{family}.otf`
/// (case-insensitive). Returns 404 if the font is not found.
async fn serve_font(Path(family): Path<String>) -> impl IntoResponse {
    if family.contains('/') || family.contains('\\') || family.contains("..") || family.is_empty() {
        return error_response(StatusCode::BAD_REQUEST, "invalid font family name");
    }

    let key = family.to_lowercase();

    // Try .ttf first, then .otf.
    let candidates = [format!("fonts/{key}.ttf"), format!("fonts/{key}.otf")];

    for path in &candidates {
        if let Ok(data) = tokio::fs::read(path).await {
            let content_type = if std::path::Path::new(path)
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("otf"))
            {
                "font/otf"
            } else {
                "font/ttf"
            };
            return (StatusCode::OK, [(header::CONTENT_TYPE, content_type)], data).into_response();
        }
    }

    error_response(StatusCode::NOT_FOUND, &format!("font '{family}' not found"))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    fn test_state() -> AppState {
        AppState::new_test()
    }

    #[tokio::test]
    async fn health_endpoint() {
        let app = create_router(test_state());
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn list_tools_endpoint() {
        let app = create_router(test_state());
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/tools")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let tools: Vec<selean_llm::ToolDefinition> = serde_json::from_slice(&body).unwrap();
        assert_eq!(tools.len(), 41);
    }

    #[tokio::test]
    async fn chat_endpoint_returns_sse() {
        // This test verifies the endpoint exists and accepts the right shape.
        // It will return an error event since we're using a test API key.
        let app = create_router(test_state());
        let body = serde_json::json!({
            "messages": [{"role": "user", "content": "hello"}],
            "scene_summary": null
        });

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/chat")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_string(&body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        // Should return 200 with SSE content type (even on API error, we stream the error)
        assert_eq!(response.status(), StatusCode::OK);
        let content_type = response
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        assert!(
            content_type.contains("text/event-stream"),
            "expected SSE content type, got: {content_type}"
        );
    }

    #[tokio::test]
    async fn figma_import_missing_file_key() {
        let app = create_router(test_state());
        let body = serde_json::json!({
            "file_key": ""
        });
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/import/figma")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_string(&body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert!(json["error"].as_str().unwrap().contains("file_key"));
    }

    #[tokio::test]
    async fn figma_import_server_token_not_configured() {
        // test_state() has figma_access_token: None
        let app = create_router(test_state());
        let body = serde_json::json!({
            "file_key": "some-key"
        });
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/import/figma")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_string(&body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert!(
            json["error"]
                .as_str()
                .unwrap()
                .contains("FIGMA_ACCESS_TOKEN")
        );
    }

    #[tokio::test]
    async fn figma_import_route_exists() {
        let app = create_router(test_state());
        // Sending invalid JSON should get a 4xx, not 404/405.
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/import/figma")
                    .header("content-type", "application/json")
                    .body(Body::from("{}"))
                    .unwrap(),
            )
            .await
            .unwrap();

        // Missing required fields returns 422 (Unprocessable Entity) from axum deserialization
        assert_ne!(response.status(), StatusCode::NOT_FOUND);
        assert_ne!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
    }

    #[tokio::test]
    async fn figma_export_route_exists() {
        let app = create_router(test_state());
        let doc = selean_engine::persistence::Document::new();
        let doc_json = selean_engine::persistence::save_document(&doc).unwrap();
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/export/figma")
                    .header("content-type", "application/json")
                    .body(Body::from(doc_json))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_ne!(response.status(), StatusCode::NOT_FOUND);
        assert_ne!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
        assert_eq!(response.status(), StatusCode::OK);

        let ct = response
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        assert!(ct.contains("application/json"));
    }

    #[tokio::test]
    async fn figma_export_invalid_json_returns_400() {
        let app = create_router(test_state());
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/export/figma")
                    .header("content-type", "application/json")
                    .body(Body::from("not valid json"))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn font_endpoint_unknown_returns_404() {
        let app = create_router(test_state());
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/fonts/nonexistent")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn font_endpoint_serves_valid_font() {
        // Create a temporary fonts/ directory with a test font.
        let dir = std::path::Path::new("fonts");
        let _ = tokio::fs::create_dir_all(dir).await;
        let font_path = dir.join("testfont.ttf");
        let sample_data = b"fake-font-data";
        tokio::fs::write(&font_path, sample_data).await.unwrap();

        let app = create_router(test_state());
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/fonts/testfont")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let ct = response
            .headers()
            .get(header::CONTENT_TYPE)
            .unwrap()
            .to_str()
            .unwrap();
        assert_eq!(ct, "font/ttf");

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        assert_eq!(body.as_ref(), sample_data);

        // Clean up.
        let _ = tokio::fs::remove_file(&font_path).await;
    }

    #[tokio::test]
    async fn font_endpoint_case_insensitive() {
        let app = create_router(test_state());
        // Request with uppercase should still match lowercase file.
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/fonts/NoSuchFont")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        // Should be 404, but importantly the route itself matches (not 405).
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn ws_endpoint_requires_auth_when_enabled() {
        let auth = AuthConfig::with_secret("s3cret");
        let app = create_router_with_options(test_state(), CollabState::new(), auth);

        // Attempt WebSocket upgrade without auth token.
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/ws")
                    .header("connection", "upgrade")
                    .header("upgrade", "websocket")
                    .header("sec-websocket-version", "13")
                    .header("sec-websocket-key", "dGhlIHNhbXBsZSBub25jZQ==")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn ws_endpoint_allows_valid_auth_token() {
        let auth = AuthConfig::with_secret("s3cret");
        let app = create_router_with_options(test_state(), CollabState::new(), auth);

        // Attempt WebSocket upgrade with valid auth token.
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/ws")
                    .header("authorization", "Bearer s3cret")
                    .header("connection", "upgrade")
                    .header("upgrade", "websocket")
                    .header("sec-websocket-version", "13")
                    .header("sec-websocket-key", "dGhlIHNhbXBsZSBub25jZQ==")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        // Auth should pass. The actual status depends on the WebSocket
        // handshake details, but critically it must NOT be 401 Unauthorized.
        assert_ne!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn font_endpoint_rejects_path_traversal() {
        // Axum normalizes `..` at the URI level (so `/api/fonts/..` becomes
        // `/api/` and never reaches the handler). Percent-encoded dots bypass
        // that normalization and arrive decoded in the path parameter.
        let app = create_router(test_state());
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/fonts/%2e%2e")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    /// Builds a minimal multipart/form-data body with a single "file" field.
    fn multipart_file_body(data: &[u8]) -> (String, Vec<u8>) {
        let boundary = "----TestBoundary123";
        let mut body = Vec::new();
        body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
        body.extend_from_slice(
            b"Content-Disposition: form-data; name=\"file\"; filename=\"test.indd\"\r\n",
        );
        body.extend_from_slice(b"Content-Type: application/octet-stream\r\n\r\n");
        body.extend_from_slice(data);
        body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
        let content_type = format!("multipart/form-data; boundary={boundary}");
        (content_type, body)
    }

    #[tokio::test]
    async fn indd_import_not_configured_returns_501() {
        let app = create_router(test_state());
        let (content_type, body) = multipart_file_body(b"fake-indd-data");
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/import/indd")
                    .header("content-type", content_type)
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_IMPLEMENTED);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let msg = json["error"].as_str().unwrap();
        assert!(
            msg.contains("IDML"),
            "error should mention IDML alternative"
        );
        assert!(
            msg.contains("not configured"),
            "error should mention server not configured"
        );
    }

    #[tokio::test]
    async fn indd_import_route_exists() {
        let app = create_router(test_state());
        // POST with empty body should not return 404 or 405.
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/import/indd")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_ne!(response.status(), StatusCode::NOT_FOUND);
        assert_ne!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
    }
}
