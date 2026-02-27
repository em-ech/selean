//! HTTP route definitions for the Selean server.
//!
//! All routes are mounted under `/api/`. Static file serving for the React
//! frontend is added at the router level.

use axum::{
    Json, Router,
    extract::{Multipart, State},
    http::{StatusCode, header},
    response::{
        IntoResponse,
        sse::{Event, KeepAlive, Sse},
    },
    routing::{get, post},
};
use std::convert::Infallible;

use crate::chat::{ChatEvent, ChatRequest, response_to_events, send_chat_request};
use crate::state::AppState;

/// Creates the Axum router with all API routes.
pub fn create_router(state: AppState) -> Router {
    Router::new()
        .route("/api/health", get(health))
        .route("/api/tools", get(list_tools))
        .route("/api/chat", post(chat_handler))
        .route("/api/import/pptx", post(import_pptx_handler))
        .route("/api/export/pptx", post(export_pptx_handler))
        .with_state(state)
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
    let result = send_chat_request(&state, &request).await;

    match result {
        Ok(response) => {
            let events = response_to_events(&response);
            let stream = tokio_stream::iter(events.into_iter().map(|event| {
                let json = serde_json::to_string(&event).unwrap_or_default();
                Ok::<_, Infallible>(Event::default().data(json))
            }));

            Sse::new(stream)
                .keep_alive(KeepAlive::default())
                .into_response()
        }
        Err(e) => {
            let error_event = ChatEvent::Error {
                message: e.to_string(),
            };
            let json = serde_json::to_string(&error_event).unwrap_or_default();
            let stream = tokio_stream::iter(std::iter::once(Ok::<_, Infallible>(
                Event::default().data(json),
            )));

            Sse::new(stream)
                .keep_alive(KeepAlive::default())
                .into_response()
        }
    }
}

/// PPTX content type for responses.
const PPTX_CONTENT_TYPE: &str =
    "application/vnd.openxmlformats-officedocument.presentationml.presentation";

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
        assert_eq!(tools.len(), 26);
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
}
