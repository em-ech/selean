//! AI chat: the request and event types shared with the web app, and the
//! HTTP round trip to the configured LLM provider.
//!
//! Provider-specific request and response formats live in
//! [`crate::provider`]. The SSE streaming to the client is handled in
//! [`super::routes`].

use futures_util::StreamExt;
use serde::{Deserialize, Serialize};

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
    /// Text chunk from Claude.
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
        /// Stop reason from Claude.
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
    let response = provider
        .request(state, &system_prompt(request), &request.messages, false)?
        .send()
        .await
        .map_err(|e| ChatError::HttpError(e.to_string()))?;

    let status = response.status();
    let body = response
        .text()
        .await
        .map_err(|e| ChatError::HttpError(e.to_string()))?;
    if !status.is_success() {
        return Err(ChatError::ApiError {
            status: status.as_u16(),
            body,
        });
    }

    provider.parse_response(&body)
}

/// Sends a chat request to the configured provider with streaming enabled
/// and returns a receiver that yields [`ChatEvent`] values as they arrive.
///
/// Spawns a background tokio task that reads the byte stream, parses it with
/// the provider's [`EventParser`], and forwards the events through an `mpsc`
/// channel. If the provider is not configured, the receiver yields a single
/// error event and no request is made.
pub fn send_chat_request_streaming(
    state: &AppState,
    request: &ChatRequest,
) -> tokio::sync::mpsc::Receiver<ChatEvent> {
    let (tx, rx) = tokio::sync::mpsc::channel(64);
    let provider = state.llm_provider.client();

    match provider.request(state, &system_prompt(request), &request.messages, true) {
        Ok(http_request) => {
            tokio::spawn(forward_stream(http_request, provider.stream_parser(), tx));
        }
        Err(e) => {
            // The channel was just created with spare capacity, so this cannot fail.
            let _ = tx.try_send(ChatEvent::Error {
                message: e.to_string(),
            });
        }
    }

    rx
}

/// Sends the request and forwards the parsed response stream to `tx`. Any
/// failure is forwarded as a single [`ChatEvent::Error`].
async fn forward_stream(
    http_request: reqwest::RequestBuilder,
    mut parser: Box<dyn EventParser>,
    tx: tokio::sync::mpsc::Sender<ChatEvent>,
) {
    if let Err(e) = pump_stream(http_request, parser.as_mut(), &tx).await {
        let _ = tx
            .send(ChatEvent::Error {
                message: e.to_string(),
            })
            .await;
    }
}

/// Outcome of forwarding events to the client.
enum Forwarded {
    /// The client is still listening.
    Delivered,
    /// The client went away; stop reading the provider's stream.
    ClientGone,
}

async fn forward_events(
    events: Vec<ChatEvent>,
    tx: &tokio::sync::mpsc::Sender<ChatEvent>,
) -> Forwarded {
    for event in events {
        if tx.send(event).await.is_err() {
            return Forwarded::ClientGone;
        }
    }
    Forwarded::Delivered
}

async fn pump_stream(
    http_request: reqwest::RequestBuilder,
    parser: &mut dyn EventParser,
    tx: &tokio::sync::mpsc::Sender<ChatEvent>,
) -> Result<(), ChatError> {
    let response = http_request
        .send()
        .await
        .map_err(|e| ChatError::HttpError(e.to_string()))?;

    if !response.status().is_success() {
        let status = response.status().as_u16();
        let body = response.text().await.unwrap_or_default();
        return Err(ChatError::ApiError { status, body });
    }

    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| ChatError::StreamError(e.to_string()))?;
        if let Forwarded::ClientGone = forward_events(parser.feed(&chunk), tx).await {
            return Ok(());
        }
    }
    forward_events(parser.finish(), tx).await;
    Ok(())
}

/// Errors from chat operations.
#[derive(Debug, thiserror::Error)]
pub enum ChatError {
    /// The server has no `ANTHROPIC_API_KEY`, so AI chat is disabled.
    #[error("AI chat is disabled: ANTHROPIC_API_KEY is not set on the server")]
    MissingApiKey,
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
    use super::*;

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
    async fn streaming_without_api_key_yields_single_error_event() {
        let mut state = AppState::new_test();
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
    async fn non_streaming_without_api_key_errors() {
        let mut state = AppState::new_test();
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
