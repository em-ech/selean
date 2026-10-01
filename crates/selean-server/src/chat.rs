//! AI chat: the request and event types shared with the web app, and the
//! HTTP round trip to the configured LLM provider.
//!
//! Provider-specific request and response formats live in
//! [`crate::provider`]. The SSE streaming to the client is handled in
//! [`super::routes`].

use futures_util::StreamExt;
use serde::{Deserialize, Serialize};

use crate::provider::Failure;
use crate::provider::sse::EventParser;
use crate::state::AppState;

/// A chat message in the conversation history.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    /// Role: `user`, `assistant`, or `tool_result`.
    pub role: String,
    /// Message content (text or structured content blocks).
    pub content: serde_json::Value,
}

/// Request body for `POST /api/chat`.
#[derive(Debug, Deserialize)]
pub struct ChatRequest {
    /// Conversation messages.
    pub messages: Vec<ChatMessage>,
    /// Current scene summary JSON (injected into system prompt).
    pub scene_summary: Option<String>,
}

/// SSE event sent to the frontend.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
pub enum ChatEvent {
    /// Text chunk from the model.
    #[serde(rename = "text")]
    Text {
        /// The text content.
        text: String,
    },
    /// Tool call that needs execution.
    #[serde(rename = "tool_use")]
    ToolUse {
        /// Tool use ID.
        id: String,
        /// Tool name.
        name: String,
        /// Tool arguments.
        input: serde_json::Value,
    },
    /// Chat turn complete.
    #[serde(rename = "done")]
    Done {
        /// Stop reason, in Claude's vocabulary (`end_turn`, `tool_use`, ...).
        stop_reason: String,
    },
    /// Error occurred.
    #[serde(rename = "error")]
    Error {
        /// Error message.
        message: String,
    },
}

/// System prompt for the design assistant.
const SYSTEM_PROMPT: &str = "\
You are a design assistant for the Selean design platform. You help users create \
and modify visual designs by manipulating nodes on a canvas.

Each node has a unique UUID, a name, a type (Frame, Text, Image, Vector, Group), \
position/size bounds, optional fill and stroke colors, opacity, blend mode, and \
other properties.

When the user asks you to make changes, use the available tools to modify the scene. \
Always explain what you are doing. If you need to understand the current state of the \
design, use get_scene_summary or get_node first.

Colors are specified as RGBA values from 0.0 to 1.0. Positions and sizes are in \
logical pixels.";

/// Builds the system prompt, including the current scene when the client
/// sent one.
fn system_prompt(request: &ChatRequest) -> String {
    let mut system = SYSTEM_PROMPT.to_string();
    if let Some(summary) = &request.scene_summary {
        system.push_str("\n\nCurrent scene state:\n");
        system.push_str(summary);
    }
    system
}

/// Sends a chat request to the configured provider and returns the complete
/// response as [`ChatEvent`] values, ending with [`ChatEvent::Done`].
///
/// # Errors
///
/// Returns an error if the provider is not configured, the HTTP request
/// fails, or the provider returns an error.
pub async fn send_chat_request(
    state: &AppState,
    request: &ChatRequest,
) -> Result<Vec<ChatEvent>, ChatError> {
    let provider = state.llm_provider.client();
    let http_request =
        provider.request(state, &system_prompt(request), &request.messages, false)?;

    let response = send_to_provider(state, http_request).await?;
    let body = response
        .text()
        .await
        .map_err(|e| ChatError::HttpError(e.to_string()))?;
    provider.parse_response(&body)
}

/// Sends a chat request to the configured provider with streaming enabled
/// and returns a receiver that yields [`ChatEvent`] values as they arrive.
///
/// Spawns a background tokio task that reads the byte stream, parses it with
/// the provider's [`EventParser`], and forwards the events through an `mpsc`
/// channel. Any failure, including a provider that is not configured or not
/// reachable, arrives as a single [`ChatEvent::Error`].
pub fn send_chat_request_streaming(
    state: &AppState,
    request: &ChatRequest,
) -> tokio::sync::mpsc::Receiver<ChatEvent> {
    let (tx, rx) = tokio::sync::mpsc::channel(64);
    let provider = state.llm_provider.client();

    match provider.request(state, &system_prompt(request), &request.messages, true) {
        Ok(http_request) => {
            let state = state.clone();
            tokio::spawn(async move {
                let mut parser = provider.stream_parser();
                if let Err(e) = forward_stream(&state, http_request, parser.as_mut(), &tx).await {
                    let _ = tx.send(error_event(&e)).await;
                }
            });
        }
        Err(e) => {
            // The channel was just created with spare capacity, so this cannot fail.
            let _ = tx.try_send(error_event(&e));
        }
    }

    rx
}

fn error_event(error: &ChatError) -> ChatEvent {
    ChatEvent::Error {
        message: error.to_string(),
    }
}

/// Sends the request and returns the response if the provider accepted it.
///
/// A failure the provider can explain (server not running, model missing,
/// key rejected) becomes [`ChatError::Provider`] with the fix in the message.
async fn send_to_provider(
    state: &AppState,
    http_request: reqwest::RequestBuilder,
) -> Result<reqwest::Response, ChatError> {
    let provider = state.llm_provider.client();

    let response = match http_request.send().await {
        Ok(response) => response,
        Err(e) => {
            let explained = e
                .is_connect()
                .then(|| provider.explain_failure(state, &Failure::Unreachable))
                .flatten();
            return Err(
                explained.map_or_else(|| ChatError::HttpError(e.to_string()), ChatError::Provider)
            );
        }
    };

    if response.status().is_success() {
        return Ok(response);
    }
    let status = response.status().as_u16();
    let body = response.text().await.unwrap_or_default();
    let failure = Failure::Status {
        status,
        body: body.clone(),
    };
    Err(provider
        .explain_failure(state, &failure)
        .map_or(ChatError::ApiError { status, body }, ChatError::Provider))
}

/// Sends the request and forwards the parsed response stream to `tx`.
/// Returns early, without error, if the client stops listening.
async fn forward_stream(
    state: &AppState,
    http_request: reqwest::RequestBuilder,
    parser: &mut dyn EventParser,
    tx: &tokio::sync::mpsc::Sender<ChatEvent>,
) -> Result<(), ChatError> {
    let response = send_to_provider(state, http_request).await?;

    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| ChatError::StreamError(e.to_string()))?;
        for event in parser.feed(&chunk) {
            if tx.send(event).await.is_err() {
                return Ok(());
            }
        }
    }
    for event in parser.finish() {
        if tx.send(event).await.is_err() {
            return Ok(());
        }
    }
    Ok(())
}

/// Errors from chat operations.
#[derive(Debug, thiserror::Error)]
pub enum ChatError {
    /// Anthropic is selected but the server has no `ANTHROPIC_API_KEY`.
    #[error(
        "AI chat is disabled: LLM_PROVIDER=anthropic needs ANTHROPIC_API_KEY, which is not set \
         on the server"
    )]
    MissingApiKey,
    /// The provider could not serve the request; the message tells the user
    /// what happened and, where known, how to fix it.
    #[error("{0}")]
    Provider(String),
    /// HTTP request failed.
    #[error("HTTP error: {0}")]
    HttpError(String),
    /// The provider returned an error status.
    #[error("API error (status {status}): {body}")]
    ApiError {
        /// HTTP status code.
        status: u16,
        /// Response body.
        body: String,
    },
    /// The response stream broke off.
    #[error("Stream error: {0}")]
    StreamError(String),
    /// Failed to parse the provider's response.
    #[error("parse error: {0}")]
    ParseError(String),
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use std::sync::{Arc, Mutex};

    use axum::http::{HeaderMap, StatusCode, header};
    use serde_json::{Value, json};

    use super::*;

    /// What the stand-in model server saw in the last request.
    #[derive(Clone, Default)]
    struct Received {
        authorization: Arc<Mutex<Option<String>>>,
        body: Arc<Mutex<Value>>,
    }

    /// Starts a local stand-in for an OpenAI-compatible server that answers
    /// `POST /v1/chat/completions` with the given status and body, and returns
    /// app state pointed at it with no API key.
    async fn keyless_state_with_model_server(
        status: StatusCode,
        content_type: &'static str,
        response_body: &'static str,
    ) -> (AppState, Received) {
        let received = Received::default();
        let recorder = received.clone();
        let app = axum::Router::new().route(
            "/v1/chat/completions",
            axum::routing::post(
                move |headers: HeaderMap, axum::Json(body): axum::Json<Value>| async move {
                    *recorder.authorization.lock().expect("lock") = headers
                        .get(header::AUTHORIZATION)
                        .map(|value| value.to_str().unwrap_or_default().to_string());
                    *recorder.body.lock().expect("lock") = body;
                    (
                        status,
                        [(header::CONTENT_TYPE, content_type)],
                        response_body,
                    )
                },
            ),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind a local port");
        let address = listener.local_addr().expect("local address");
        tokio::spawn(async move {
            axum::serve(listener, app).await.expect("serve");
        });

        let mut state = AppState::new_test();
        state.llm_base_url = Some(Arc::from(format!("http://{address}/v1")));
        (state, received)
    }

    /// App state pointed at a local port where nothing is listening.
    async fn keyless_state_with_no_model_server() -> AppState {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind a local port");
        let address = listener.local_addr().expect("local address");
        drop(listener);

        let mut state = AppState::new_test();
        state.llm_base_url = Some(Arc::from(format!("http://{address}/v1")));
        state
    }

    async fn collect(mut rx: tokio::sync::mpsc::Receiver<ChatEvent>) -> Vec<ChatEvent> {
        let mut events = Vec::new();
        while let Some(event) = rx.recv().await {
            events.push(event);
        }
        events
    }

    const TOOL_CALL_STREAM: &str = "data: {\"choices\":[{\"index\":0,\"delta\":{\"content\":\"On it.\"}}]}\n\n\
data: {\"choices\":[{\"index\":0,\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"call_1\",\"function\":{\"name\":\"set_fill\",\"arguments\":\"{\\\"r\\\":\"}}]}}]}\n\n\
data: {\"choices\":[{\"index\":0,\"delta\":{\"tool_calls\":[{\"index\":0,\"function\":{\"arguments\":\"1.0}\"}}]},\"finish_reason\":\"tool_calls\"}]}\n\n\
data: [DONE]\n\n";

    const MODEL_NOT_FOUND: &str = r#"{"error":{"message":"model \"qwen2.5:7b\" not found, try pulling it first","type":"api_error","param":null,"code":null}}"#;

    #[tokio::test]
    async fn keyless_streaming_chat_reaches_the_server_and_yields_client_events() {
        let (state, received) =
            keyless_state_with_model_server(StatusCode::OK, "text/event-stream", TOOL_CALL_STREAM)
                .await;

        let events = collect(send_chat_request_streaming(&state, &hello_request())).await;

        assert_eq!(events.len(), 3, "got: {events:?}");
        assert!(matches!(&events[0], ChatEvent::Text { text } if text == "On it."));
        match &events[1] {
            ChatEvent::ToolUse { id, name, input } => {
                assert_eq!(id, "call_1");
                assert_eq!(name, "set_fill");
                assert_eq!(input, &json!({"r": 1.0}));
            }
            other => panic!("expected ToolUse, got {other:?}"),
        }
        assert!(matches!(&events[2], ChatEvent::Done { stop_reason } if stop_reason == "tool_use"));

        assert!(received.authorization.lock().expect("lock").is_none());
        let body = received.body.lock().expect("lock");
        assert_eq!(body["model"], "qwen2.5:7b");
        assert_eq!(body["stream"], true);
        assert_eq!(body["messages"][0]["role"], "system");
        assert_eq!(
            body["messages"][1],
            json!({"role": "user", "content": "hello"})
        );
        assert_eq!(body["tools"][0]["type"], "function");
    }

    #[tokio::test]
    async fn keyless_non_streaming_chat_returns_client_events() {
        let (state, received) = keyless_state_with_model_server(
            StatusCode::OK,
            "application/json",
            r#"{"choices":[{"message":{"role":"assistant","content":"Hi there."},"finish_reason":"stop"}]}"#,
        )
        .await;

        let events = send_chat_request(&state, &hello_request())
            .await
            .expect("chat succeeds without a key");

        assert_eq!(events.len(), 2);
        assert!(matches!(&events[0], ChatEvent::Text { text } if text == "Hi there."));
        assert!(matches!(&events[1], ChatEvent::Done { stop_reason } if stop_reason == "end_turn"));
        assert!(received.authorization.lock().expect("lock").is_none());
        assert!(received.body.lock().expect("lock").get("stream").is_none());
    }

    #[tokio::test]
    async fn api_key_is_sent_as_a_bearer_token_when_set() {
        let (mut state, received) = keyless_state_with_model_server(
            StatusCode::OK,
            "application/json",
            r#"{"choices":[{"message":{"content":"ok"},"finish_reason":"stop"}]}"#,
        )
        .await;
        state.api_key = Some(Arc::from("test-key"));

        send_chat_request(&state, &hello_request())
            .await
            .expect("chat succeeds");

        assert_eq!(
            received.authorization.lock().expect("lock").as_deref(),
            Some("Bearer test-key")
        );
    }

    #[tokio::test]
    async fn streaming_with_no_model_server_yields_one_actionable_error() {
        let state = keyless_state_with_no_model_server().await;
        let base_url = state.llm_base_url.clone().expect("base URL is set");

        let events = collect(send_chat_request_streaming(&state, &hello_request())).await;

        assert_eq!(events.len(), 1, "got: {events:?}");
        match &events[0] {
            ChatEvent::Error { message } => {
                assert!(message.contains("could not reach"), "got: {message}");
                assert!(message.contains(&*base_url), "got: {message}");
                assert!(message.contains("LLM_BASE_URL"), "got: {message}");
            }
            other => panic!("expected Error, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn non_streaming_with_no_model_server_is_an_actionable_error() {
        let state = keyless_state_with_no_model_server().await;

        let err = send_chat_request(&state, &hello_request())
            .await
            .expect_err("nothing is listening");

        match err {
            ChatError::Provider(message) => {
                assert!(message.contains("could not reach"), "got: {message}");
            }
            other => panic!("expected a provider error, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn streaming_with_a_model_that_is_not_downloaded_yields_one_actionable_error() {
        let (state, _) = keyless_state_with_model_server(
            StatusCode::NOT_FOUND,
            "application/json",
            MODEL_NOT_FOUND,
        )
        .await;

        let events = collect(send_chat_request_streaming(&state, &hello_request())).await;

        assert_eq!(events.len(), 1, "got: {events:?}");
        match &events[0] {
            ChatEvent::Error { message } => {
                assert!(message.contains("qwen2.5:7b"), "got: {message}");
                assert!(message.contains("was not found"), "got: {message}");
                assert!(message.contains("LLM_MODEL"), "got: {message}");
            }
            other => panic!("expected Error, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn non_streaming_with_a_model_that_is_not_downloaded_is_an_actionable_error() {
        let (state, _) = keyless_state_with_model_server(
            StatusCode::NOT_FOUND,
            "application/json",
            MODEL_NOT_FOUND,
        )
        .await;

        let err = send_chat_request(&state, &hello_request())
            .await
            .expect_err("the model is missing");
        assert!(matches!(err, ChatError::Provider(message) if message.contains("LLM_MODEL")));
    }

    #[tokio::test]
    async fn unexplained_server_error_keeps_status_and_body() {
        let (state, _) = keyless_state_with_model_server(
            StatusCode::INTERNAL_SERVER_ERROR,
            "text/plain",
            "boom",
        )
        .await;

        let err = send_chat_request(&state, &hello_request())
            .await
            .expect_err("server error");
        assert!(matches!(err, ChatError::ApiError { status: 500, body } if body == "boom"));

        let events = collect(send_chat_request_streaming(&state, &hello_request())).await;
        assert_eq!(events.len(), 1);
        assert!(matches!(&events[0], ChatEvent::Error { message }
            if message == "API error (status 500): boom"));
    }

    fn hello_request() -> ChatRequest {
        ChatRequest {
            messages: vec![ChatMessage {
                role: "user".to_string(),
                content: serde_json::json!("hello"),
            }],
            scene_summary: None,
        }
    }

    #[tokio::test]
    async fn anthropic_streaming_without_api_key_yields_single_error_event() {
        let mut state = AppState::new_test_anthropic();
        state.api_key = None;

        let mut rx = send_chat_request_streaming(&state, &hello_request());

        match rx.recv().await {
            Some(ChatEvent::Error { message }) => {
                assert!(message.contains("ANTHROPIC_API_KEY"), "got: {message}");
            }
            other => panic!("expected an error event, got {other:?}"),
        }
        assert!(rx.recv().await.is_none());
    }

    #[tokio::test]
    async fn anthropic_non_streaming_without_api_key_errors() {
        let mut state = AppState::new_test_anthropic();
        state.api_key = None;

        let err = send_chat_request(&state, &hello_request())
            .await
            .expect_err("missing key must fail");
        assert!(matches!(err, ChatError::MissingApiKey));
    }

    #[test]
    fn system_prompt_includes_scene_summary() {
        let request = ChatRequest {
            messages: vec![],
            scene_summary: Some("{\"nodes\": []}".to_string()),
        };

        let system = system_prompt(&request);
        assert!(system.contains("Current scene state:"));
        assert!(system.contains("{\"nodes\": []}"));
    }

    #[test]
    fn system_prompt_without_scene_summary() {
        let system = system_prompt(&hello_request());
        assert!(!system.contains("Current scene state:"));
    }

    #[test]
    fn chat_message_roundtrip() {
        let msg = ChatMessage {
            role: "user".to_string(),
            content: serde_json::json!("Hello"),
        };
        let json = serde_json::to_string(&msg).unwrap();
        let back: ChatMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(back.role, "user");
    }

    #[test]
    fn chat_event_serializes_with_type_tag() {
        let event = ChatEvent::Text {
            text: "Hi".to_string(),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"type\":\"text\""));
    }
}
