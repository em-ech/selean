//! Claude API chat integration.
//!
//! Handles building the Claude API request with tool definitions and
//! processing the response. The actual SSE streaming to the client is
//! handled in [`super::routes`].

use futures_util::StreamExt;
use selean_llm::{ToolDefinition, all_tools};
use serde::{Deserialize, Serialize};

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

/// A content block from Claude's response.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ContentBlock {
    /// Text content.
    #[serde(rename = "text")]
    Text {
        /// The text content.
        text: String,
    },
    /// Tool use request.
    #[serde(rename = "tool_use")]
    ToolUse {
        /// Tool use ID.
        id: String,
        /// Tool name.
        name: String,
        /// Tool arguments as JSON.
        input: serde_json::Value,
    },
}

/// Claude API response (non-streaming).
#[derive(Debug, Deserialize)]
pub struct ClaudeResponse {
    /// Response content blocks.
    pub content: Vec<ContentBlock>,
    /// Stop reason.
    pub stop_reason: Option<String>,
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

/// Builds the Claude API request body.
pub fn build_claude_request(state: &AppState, request: &ChatRequest) -> serde_json::Value {
    let tools: Vec<ToolDefinition> = all_tools();

    let mut system = SYSTEM_PROMPT.to_string();
    if let Some(summary) = &request.scene_summary {
        system.push_str("\n\nCurrent scene state:\n");
        system.push_str(summary);
    }

    serde_json::json!({
        "model": &*state.model,
        "max_tokens": 4096,
        "system": system,
        "messages": request.messages,
        "tools": tools,
    })
}

/// Sends a chat request to Claude and returns the response.
///
/// # Errors
///
/// Returns an error if no API key is configured, the HTTP request fails, or
/// Claude returns an error.
pub async fn send_chat_request(
    state: &AppState,
    request: &ChatRequest,
) -> Result<ClaudeResponse, ChatError> {
    let api_key = state.api_key.as_deref().ok_or(ChatError::MissingApiKey)?;
    let body = build_claude_request(state, request);

    let response = state
        .http_client
        .post("https://api.anthropic.com/v1/messages")
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| ChatError::HttpError(e.to_string()))?;

    if !response.status().is_success() {
        let status = response.status().as_u16();
        let body = response.text().await.unwrap_or_default();
        return Err(ChatError::ApiError { status, body });
    }

    let claude_response: ClaudeResponse = response
        .json()
        .await
        .map_err(|e| ChatError::ParseError(e.to_string()))?;

    Ok(claude_response)
}

/// Converts a Claude response into a sequence of [`ChatEvent`] values.
pub fn response_to_events(response: &ClaudeResponse) -> Vec<ChatEvent> {
    let mut events = Vec::new();

    for block in &response.content {
        match block {
            ContentBlock::Text { text } => {
                events.push(ChatEvent::Text { text: text.clone() });
            }
            ContentBlock::ToolUse { id, name, input } => {
                events.push(ChatEvent::ToolUse {
                    id: id.clone(),
                    name: name.clone(),
                    input: input.clone(),
                });
            }
        }
    }

    events.push(ChatEvent::Done {
        stop_reason: response
            .stop_reason
            .clone()
            .unwrap_or_else(|| "end_turn".to_string()),
    });

    events
}

/// Sends a chat request to Claude with streaming enabled and returns a
/// receiver that yields [`ChatEvent`] values as they arrive.
///
/// Spawns a background tokio task that reads the byte stream from Claude,
/// parses SSE events via [`crate::stream::StreamParser`], and forwards
/// parsed events through an `mpsc` channel. Without an API key the receiver
/// yields a single error event and no request is made.
pub fn send_chat_request_streaming(
    state: &AppState,
    request: &ChatRequest,
) -> tokio::sync::mpsc::Receiver<ChatEvent> {
    let mut body = build_claude_request(state, request);
    // build_claude_request always returns a JSON object, so as_object_mut is safe.
    if let Some(obj) = body.as_object_mut() {
        obj.insert("stream".to_string(), serde_json::json!(true));
    }

    let client = state.http_client.clone();
    let (tx, rx) = tokio::sync::mpsc::channel(64);

    let Some(api_key) = state.api_key.clone() else {
        // The channel was just created with spare capacity, so this cannot fail.
        let _ = tx.try_send(ChatEvent::Error {
            message: ChatError::MissingApiKey.to_string(),
        });
        return rx;
    };

    tokio::spawn(async move {
        let response = client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", &*api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await;

        let response = match response {
            Ok(r) => r,
            Err(e) => {
                let _ = tx
                    .send(ChatEvent::Error {
                        message: format!("HTTP error: {e}"),
                    })
                    .await;
                return;
            }
        };

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body_text = response.text().await.unwrap_or_default();
            let _ = tx
                .send(ChatEvent::Error {
                    message: format!("API error (status {status}): {body_text}"),
                })
                .await;
            return;
        }

        let mut stream = response.bytes_stream();
        let mut parser = crate::stream::StreamParser::new();

        while let Some(chunk_result) = stream.next().await {
            let chunk = match chunk_result {
                Ok(c) => c,
                Err(e) => {
                    let _ = tx
                        .send(ChatEvent::Error {
                            message: format!("Stream error: {e}"),
                        })
                        .await;
                    return;
                }
            };

            let events = parser.feed(&chunk);
            for event in events {
                if tx.send(event).await.is_err() {
                    return;
                }
            }
        }
    });

    rx
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
    /// Claude API returned an error status.
    #[error("API error (status {status}): {body}")]
    ApiError {
        /// HTTP status code.
        status: u16,
        /// Response body.
        body: String,
    },
    /// Failed to parse Claude's response.
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
    fn build_claude_request_includes_tools() {
        let state = AppState::new_test();
        let request = ChatRequest {
            messages: vec![ChatMessage {
                role: "user".to_string(),
                content: serde_json::json!("Make it red"),
            }],
            scene_summary: None,
        };

        let body = build_claude_request(&state, &request);
        assert!(body["tools"].is_array());
        assert!(!body["tools"].as_array().unwrap().is_empty());
        assert_eq!(body["model"], "claude-sonnet-4-6");
    }

    #[test]
    fn build_claude_request_includes_scene_summary() {
        let state = AppState::new_test();
        let request = ChatRequest {
            messages: vec![],
            scene_summary: Some("{\"nodes\": []}".to_string()),
        };

        let body = build_claude_request(&state, &request);
        let system = body["system"].as_str().unwrap();
        assert!(system.contains("Current scene state:"));
        assert!(system.contains("{\"nodes\": []}"));
    }

    #[test]
    fn build_claude_request_without_scene_summary() {
        let state = AppState::new_test();
        let request = ChatRequest {
            messages: vec![],
            scene_summary: None,
        };

        let body = build_claude_request(&state, &request);
        let system = body["system"].as_str().unwrap();
        assert!(!system.contains("Current scene state:"));
    }

    #[test]
    fn response_to_events_text_only() {
        let response = ClaudeResponse {
            content: vec![ContentBlock::Text {
                text: "I made it red.".to_string(),
            }],
            stop_reason: Some("end_turn".to_string()),
        };

        let events = response_to_events(&response);
        assert_eq!(events.len(), 2);
        match &events[0] {
            ChatEvent::Text { text } => assert_eq!(text, "I made it red."),
            _ => panic!("expected Text event"),
        }
        match &events[1] {
            ChatEvent::Done { stop_reason } => assert_eq!(stop_reason, "end_turn"),
            _ => panic!("expected Done event"),
        }
    }

    #[test]
    fn response_to_events_with_tool_use() {
        let response = ClaudeResponse {
            content: vec![
                ContentBlock::Text {
                    text: "Setting the fill.".to_string(),
                },
                ContentBlock::ToolUse {
                    id: "tool_123".to_string(),
                    name: "set_fill".to_string(),
                    input: serde_json::json!({"node_id": "abc", "r": 1.0}),
                },
            ],
            stop_reason: Some("tool_use".to_string()),
        };

        let events = response_to_events(&response);
        assert_eq!(events.len(), 3);
        assert!(matches!(&events[0], ChatEvent::Text { .. }));
        match &events[1] {
            ChatEvent::ToolUse { id, name, .. } => {
                assert_eq!(id, "tool_123");
                assert_eq!(name, "set_fill");
            }
            _ => panic!("expected ToolUse event"),
        }
        match &events[2] {
            ChatEvent::Done { stop_reason } => assert_eq!(stop_reason, "tool_use"),
            _ => panic!("expected Done event"),
        }
    }

    #[test]
    fn response_to_events_default_stop_reason() {
        let response = ClaudeResponse {
            content: vec![],
            stop_reason: None,
        };

        let events = response_to_events(&response);
        assert_eq!(events.len(), 1);
        match &events[0] {
            ChatEvent::Done { stop_reason } => assert_eq!(stop_reason, "end_turn"),
            _ => panic!("expected Done event"),
        }
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
    fn content_block_text_roundtrip() {
        let block = ContentBlock::Text {
            text: "Hello".to_string(),
        };
        let json = serde_json::to_string(&block).unwrap();
        let back: ContentBlock = serde_json::from_str(&json).unwrap();
        match back {
            ContentBlock::Text { text } => assert_eq!(text, "Hello"),
            ContentBlock::ToolUse { .. } => panic!("wrong variant"),
        }
    }

    #[test]
    fn content_block_tool_use_roundtrip() {
        let block = ContentBlock::ToolUse {
            id: "id1".to_string(),
            name: "set_fill".to_string(),
            input: serde_json::json!({"r": 1.0}),
        };
        let json = serde_json::to_string(&block).unwrap();
        let back: ContentBlock = serde_json::from_str(&json).unwrap();
        match back {
            ContentBlock::ToolUse { id, name, .. } => {
                assert_eq!(id, "id1");
                assert_eq!(name, "set_fill");
            }
            ContentBlock::Text { .. } => panic!("wrong variant"),
        }
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
