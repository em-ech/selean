//! HTTP route definitions for the Selean server.
//!
//! All routes are mounted under `/api/`. Static file serving for the React
//! frontend is added at the router level.

use axum::{
    Json, Router,
    extract::State,
    response::{
        sse::{Event, KeepAlive, Sse},
        IntoResponse,
    },
    routing::{get, post},
};
use std::convert::Infallible;

use crate::chat::{ChatEvent, ChatRequest, send_chat_request, response_to_events};
use crate::state::AppState;

/// Creates the Axum router with all API routes.
pub fn create_router(state: AppState) -> Router {
    Router::new()
        .route("/api/health", get(health))
        .route("/api/tools", get(list_tools))
        .route("/api/chat", post(chat_handler))
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
        let tools: Vec<selean_llm::ToolDefinition> =
            serde_json::from_slice(&body).unwrap();
        assert_eq!(tools.len(), 13);
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
