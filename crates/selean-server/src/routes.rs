//! HTTP route definitions for the Selean server.
//!
//! All routes are mounted under `/api/`. Static file serving for the React
//! frontend is added at the router level.

use axum::{
    Json, Router,
    extract::{Multipart, Path, State},
    http::{StatusCode, header},
    response::{
        IntoResponse,
        sse::{Event, KeepAlive, Sse},
    },
    routing::{get, post},
};
use std::convert::Infallible;
use tokio_stream::StreamExt;

use crate::chat::{ChatRequest, send_chat_request_streaming};
use crate::collab::ws_handler::{CollabState, ws_handler};
use crate::state::AppState;

/// Creates the Axum router with all API routes.
pub fn create_router(state: AppState) -> Router {
    create_router_with_collab(state, CollabState::new())
}

/// Creates the Axum router with explicit collab state (for testing).
pub fn create_router_with_collab(state: AppState, collab_state: CollabState) -> Router {
    let collab_routes = Router::new()
        .route("/api/ws", get(ws_handler))
        .with_state(collab_state);

    Router::new()
        .route("/api/health", get(health))
        .route("/api/tools", get(list_tools))
        .route("/api/chat", post(chat_handler))
        .route("/api/import/pptx", post(import_pptx_handler))
        .route("/api/export/pptx", post(export_pptx_handler))
        .route("/api/import/idml", post(import_idml_handler))
        .route("/api/export/idml", post(export_idml_handler))
        .route("/api/import/figma", post(import_figma_handler))
        .route("/api/fonts/{family}", get(serve_font))
        .with_state(state)
        .merge(collab_routes)
}

/// Health check endpoint.
async fn health() -> &'static str {
    "ok"
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

/// PPTX content type for responses.
const PPTX_CONTENT_TYPE: &str =
    "application/vnd.openxmlformats-officedocument.presentationml.presentation";

/// IDML content type for responses.
const IDML_CONTENT_TYPE: &str = "application/vnd.adobe.indesign-idml-package";

/// Handles PPTX import. Accepts multipart form data with a `file` field
/// containing the `.pptx` bytes. Returns the parsed `Document` as JSON.
async fn import_pptx_handler(mut multipart: Multipart) -> impl IntoResponse {
    while let Ok(Some(field)) = multipart.next_field().await {
        let name = field.name().unwrap_or("").to_string();
        if name == "file" {
            let bytes = match field.bytes().await {
                Ok(b) => b,
                Err(e) => {
                    return (
                        StatusCode::BAD_REQUEST,
                        Json(serde_json::json!({ "error": format!("failed to read file: {e}") })),
                    )
                        .into_response();
                }
            };

            return match selean_pptx::import_pptx(&bytes) {
                Ok(doc) => {
                    let json = match selean_engine::persistence::save_document(&doc) {
                        Ok(j) => j,
                        Err(e) => {
                            return (
                                StatusCode::INTERNAL_SERVER_ERROR,
                                Json(serde_json::json!({
                                    "error": format!("serialization failed: {e}")
                                })),
                            )
                                .into_response();
                        }
                    };

                    (
                        StatusCode::OK,
                        [(header::CONTENT_TYPE, "application/json")],
                        json,
                    )
                        .into_response()
                }
                Err(e) => (
                    StatusCode::BAD_REQUEST,
                    Json(serde_json::json!({ "error": format!("pptx import failed: {e}") })),
                )
                    .into_response(),
            };
        }
    }

    (
        StatusCode::BAD_REQUEST,
        Json(serde_json::json!({ "error": "missing 'file' field in multipart form" })),
    )
        .into_response()
}

/// Handles PPTX export. Accepts a `Document` as JSON body and returns the
/// `.pptx` bytes with the appropriate content type.
async fn export_pptx_handler(body: axum::body::Bytes) -> impl IntoResponse {
    let doc = match selean_engine::persistence::load_document(&String::from_utf8_lossy(&body)) {
        Ok(d) => d,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "error": format!("invalid document JSON: {e}") })),
            )
                .into_response();
        }
    };

    match selean_pptx::export_pptx(&doc) {
        Ok(bytes) => (
            StatusCode::OK,
            [
                (header::CONTENT_TYPE, PPTX_CONTENT_TYPE),
                (
                    header::CONTENT_DISPOSITION,
                    "attachment; filename=\"export.pptx\"",
                ),
            ],
            bytes,
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": format!("pptx export failed: {e}") })),
        )
            .into_response(),
    }
}

/// Handles IDML import. Accepts multipart form data with a `file` field
/// containing the `.idml` bytes. Returns the parsed `Document` as JSON.
async fn import_idml_handler(mut multipart: Multipart) -> impl IntoResponse {
    while let Ok(Some(field)) = multipart.next_field().await {
        let name = field.name().unwrap_or("").to_string();
        if name == "file" {
            let bytes = match field.bytes().await {
                Ok(b) => b,
                Err(e) => {
                    return (
                        StatusCode::BAD_REQUEST,
                        Json(serde_json::json!({ "error": format!("failed to read file: {e}") })),
                    )
                        .into_response();
                }
            };

            return match selean_idml::import_idml(&bytes) {
                Ok(doc) => {
                    let json = match selean_engine::persistence::save_document(&doc) {
                        Ok(j) => j,
                        Err(e) => {
                            return (
                                StatusCode::INTERNAL_SERVER_ERROR,
                                Json(serde_json::json!({
                                    "error": format!("serialization failed: {e}")
                                })),
                            )
                                .into_response();
                        }
                    };

                    (
                        StatusCode::OK,
                        [(header::CONTENT_TYPE, "application/json")],
                        json,
                    )
                        .into_response()
                }
                Err(e) => (
                    StatusCode::BAD_REQUEST,
                    Json(serde_json::json!({ "error": format!("idml import failed: {e}") })),
                )
                    .into_response(),
            };
        }
    }

    (
        StatusCode::BAD_REQUEST,
        Json(serde_json::json!({ "error": "missing 'file' field in multipart form" })),
    )
        .into_response()
}

/// Handles IDML export. Accepts a `Document` as JSON body and returns the
/// `.idml` bytes with the appropriate content type.
async fn export_idml_handler(body: axum::body::Bytes) -> impl IntoResponse {
    let doc = match selean_engine::persistence::load_document(&String::from_utf8_lossy(&body)) {
        Ok(d) => d,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "error": format!("invalid document JSON: {e}") })),
            )
                .into_response();
        }
    };

    match selean_idml::export_idml(&doc) {
        Ok(bytes) => (
            StatusCode::OK,
            [
                (header::CONTENT_TYPE, IDML_CONTENT_TYPE),
                (
                    header::CONTENT_DISPOSITION,
                    "attachment; filename=\"export.idml\"",
                ),
            ],
            bytes,
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": format!("idml export failed: {e}") })),
        )
            .into_response(),
    }
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
) -> impl IntoResponse {
    if request.file_key.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "file_key is required" })),
        )
            .into_response();
    }

    let Some(ref access_token) = state.figma_access_token else {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": "FIGMA_ACCESS_TOKEN not configured on server" })),
        )
            .into_response();
    };

    match selean_figma::import_figma(&state.http_client, access_token, &request.file_key).await {
        Ok(doc) => {
            let json = match selean_engine::persistence::save_document(&doc) {
                Ok(j) => j,
                Err(e) => {
                    return (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(serde_json::json!({
                            "error": format!("serialization failed: {e}")
                        })),
                    )
                        .into_response();
                }
            };

            (
                StatusCode::OK,
                [(header::CONTENT_TYPE, "application/json")],
                json,
            )
                .into_response()
        }
        Err(e) => {
            let status = match &e {
                selean_figma::FigmaError::Api { status: 403, .. } => StatusCode::FORBIDDEN,
                selean_figma::FigmaError::Api { status: 404, .. } => StatusCode::NOT_FOUND,
                _ => StatusCode::BAD_REQUEST,
            };
            (
                status,
                Json(serde_json::json!({ "error": format!("figma import failed: {e}") })),
            )
                .into_response()
        }
    }
}

/// Serves a font file by family name from the `fonts/` directory.
///
/// Font files are looked up as `fonts/{family}.ttf` or `fonts/{family}.otf`
/// (case-insensitive). Returns 404 if the font is not found.
async fn serve_font(Path(family): Path<String>) -> impl IntoResponse {
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

    (
        StatusCode::NOT_FOUND,
        Json(serde_json::json!({ "error": format!("font '{family}' not found") })),
    )
        .into_response()
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
}
