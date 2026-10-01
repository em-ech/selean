//! Claude API provider.
//!
//! The web app speaks Claude's message format, so messages and tool
//! definitions are sent as they are and the response maps one to one onto
//! [`ChatEvent`] values.

use selean_llm::all_tools;
use serde::{Deserialize, Serialize};

use super::anthropic_stream::StreamParser;
use super::sse::EventParser;
use super::{ChatProvider, Failure, MAX_OUTPUT_TOKENS};
use crate::chat::{ChatError, ChatEvent, ChatMessage};
use crate::state::AppState;

const MESSAGES_URL: &str = "https://api.anthropic.com/v1/messages";
const API_VERSION: &str = "2023-06-01";

/// Claude model used when `ANTHROPIC_MODEL` is not set.
pub const DEFAULT_MODEL: &str = "claude-sonnet-4-6";

/// The Claude API provider.
pub struct Anthropic;

impl ChatProvider for Anthropic {
    fn request(
        &self,
        state: &AppState,
        system: &str,
        messages: &[ChatMessage],
        stream: bool,
    ) -> Result<reqwest::RequestBuilder, ChatError> {
        let api_key = state.api_key.as_deref().ok_or(ChatError::MissingApiKey)?;

        Ok(state
            .http_client
            .post(MESSAGES_URL)
            .header("x-api-key", api_key)
            .header("anthropic-version", API_VERSION)
            .json(&build_request(&state.model, system, messages, stream)))
    }

    fn parse_response(&self, body: &str) -> Result<Vec<ChatEvent>, ChatError> {
        let response: ClaudeResponse =
            serde_json::from_str(body).map_err(|e| ChatError::ParseError(e.to_string()))?;
        Ok(response_to_events(&response))
    }

    fn stream_parser(&self) -> Box<dyn EventParser> {
        Box::new(StreamParser::new())
    }

    /// Claude API errors are passed through as they are.
    fn explain_failure(&self, _state: &AppState, _failure: &Failure) -> Option<String> {
        None
    }
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

/// Builds the Claude API request body.
pub fn build_request(
    model: &str,
    system: &str,
    messages: &[ChatMessage],
    stream: bool,
) -> serde_json::Value {
    let mut body = serde_json::json!({
        "model": model,
        "max_tokens": MAX_OUTPUT_TOKENS,
        "system": system,
        "messages": messages,
        "tools": all_tools(),
    });
    if stream {
        body["stream"] = serde_json::json!(true);
    }
    body
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

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn user_message(text: &str) -> ChatMessage {
        ChatMessage {
            role: "user".to_string(),
            content: serde_json::json!(text),
        }
    }

    #[test]
    fn build_request_includes_tools_model_and_system() {
        let body = build_request(
            "claude-sonnet-4-6",
            "be helpful",
            &[user_message("Make it red")],
            false,
        );
        assert!(body["tools"].is_array());
        assert!(!body["tools"].as_array().unwrap().is_empty());
        assert_eq!(body["model"], "claude-sonnet-4-6");
        assert_eq!(body["system"], "be helpful");
        assert_eq!(body["messages"][0]["content"], "Make it red");
        assert!(body.get("stream").is_none());
    }

    #[test]
    fn build_request_sets_stream_flag_when_streaming() {
        let body = build_request("claude-sonnet-4-6", "", &[], true);
        assert_eq!(body["stream"], true);
    }

    #[test]
    fn request_targets_the_messages_api_with_key_and_version_headers() {
        let state = AppState::new_test_anthropic();
        let request = Anthropic
            .request(&state, "system", &[user_message("hi")], false)
            .expect("key is configured")
            .build()
            .expect("valid request");

        assert_eq!(request.url().as_str(), MESSAGES_URL);
        assert_eq!(request.headers()["x-api-key"], "test-key");
        assert_eq!(request.headers()["anthropic-version"], API_VERSION);
    }

    #[test]
    fn parse_response_maps_a_claude_body_to_events() {
        let body = r#"{"content":[{"type":"text","text":"Hi"}],"stop_reason":"end_turn"}"#;
        let events = Anthropic.parse_response(body).expect("valid body");
        assert_eq!(events.len(), 2);
        assert!(matches!(&events[0], ChatEvent::Text { text } if text == "Hi"));
        assert!(matches!(&events[1], ChatEvent::Done { stop_reason } if stop_reason == "end_turn"));
    }

    #[test]
    fn parse_response_rejects_a_body_that_is_not_a_claude_response() {
        let err = Anthropic
            .parse_response("not json")
            .expect_err("must not parse");
        assert!(matches!(err, ChatError::ParseError(_)));
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
}
