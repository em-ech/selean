//! Stateful SSE parser for Claude streaming responses.
//!
//! Converts raw byte chunks from the Claude streaming API into [`ChatEvent`]
//! values. Decoupled from async for testability: callers feed chunks via
//! [`StreamParser::feed`] and receive parsed events synchronously.

use std::collections::HashMap;

use serde::Deserialize;

use crate::chat::ChatEvent;

/// Tracks the type of content block currently being accumulated.
enum ActiveBlock {
    Text,
    ToolUse {
        id: String,
        name: String,
        json_fragments: Vec<String>,
    },
}

/// Header payload for `content_block_start` events.
#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
enum ContentBlockHeader {
    #[serde(rename = "text")]
    Text {
        #[allow(dead_code)]
        text: String,
    },
    #[serde(rename = "tool_use")]
    ToolUse { id: String, name: String },
}

/// Delta payload for `content_block_delta` events.
#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
enum DeltaPayload {
    #[serde(rename = "text_delta")]
    TextDelta { text: String },
    #[serde(rename = "input_json_delta")]
    InputJsonDelta { partial_json: String },
}

/// Payload for `message_delta` events.
#[derive(Debug, Deserialize)]
struct MessageDeltaPayload {
    stop_reason: Option<String>,
}

/// Top-level Claude SSE event envelope.
#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
enum ClaudeStreamEvent {
    #[serde(rename = "content_block_start")]
    ContentBlockStart {
        index: usize,
        content_block: ContentBlockHeader,
    },
    #[serde(rename = "content_block_delta")]
    ContentBlockDelta { index: usize, delta: DeltaPayload },
    #[serde(rename = "content_block_stop")]
    ContentBlockStop { index: usize },
    #[serde(rename = "message_delta")]
    MessageDelta { delta: MessageDeltaPayload },
    #[serde(other)]
    Other,
}

/// Synchronous SSE parser for the Claude streaming protocol.
///
/// Feed raw byte chunks from `reqwest::Response::bytes_stream()` and
/// receive [`ChatEvent`] values as they become available.
pub struct StreamParser {
    active_blocks: HashMap<usize, ActiveBlock>,
    line_buffer: String,
}

impl Default for StreamParser {
    fn default() -> Self {
        Self::new()
    }
}

impl StreamParser {
    /// Creates a new parser with empty state.
    pub fn new() -> Self {
        Self {
            active_blocks: HashMap::new(),
            line_buffer: String::new(),
        }
    }

    /// Feeds a raw byte chunk from the HTTP response stream and returns
    /// any [`ChatEvent`] values that can be emitted from the data received
    /// so far.
    pub fn feed(&mut self, chunk: &[u8]) -> Vec<ChatEvent> {
        let text = String::from_utf8_lossy(chunk);
        self.line_buffer.push_str(&text);

        let mut events = Vec::new();
        loop {
            let Some(newline_pos) = self.line_buffer.find('\n') else {
                break;
            };
            let line = self.line_buffer[..newline_pos].to_string();
            self.line_buffer = self.line_buffer[newline_pos + 1..].to_string();

            if let Some(data) = line.strip_prefix("data: ") {
                let data = data.trim();
                if data.is_empty() {
                    continue;
                }
                self.process_data(data, &mut events);
            }
        }

        events
    }

    fn process_data(&mut self, data: &str, events: &mut Vec<ChatEvent>) {
        let Ok(stream_event) = serde_json::from_str::<ClaudeStreamEvent>(data) else {
            return;
        };

        match stream_event {
            ClaudeStreamEvent::ContentBlockStart {
                index,
                content_block,
            } => match content_block {
                ContentBlockHeader::Text { .. } => {
                    self.active_blocks.insert(index, ActiveBlock::Text);
                }
                ContentBlockHeader::ToolUse { id, name } => {
                    self.active_blocks.insert(
                        index,
                        ActiveBlock::ToolUse {
                            id,
                            name,
                            json_fragments: Vec::new(),
                        },
                    );
                }
            },

            ClaudeStreamEvent::ContentBlockDelta { index, delta } => match delta {
                DeltaPayload::TextDelta { text } => {
                    events.push(ChatEvent::Text { text });
                }
                DeltaPayload::InputJsonDelta { partial_json } => {
                    if let Some(ActiveBlock::ToolUse { json_fragments, .. }) =
                        self.active_blocks.get_mut(&index)
                    {
                        json_fragments.push(partial_json);
                    }
                }
            },

            ClaudeStreamEvent::ContentBlockStop { index } => {
                if let Some(ActiveBlock::ToolUse {
                    id,
                    name,
                    json_fragments,
                }) = self.active_blocks.remove(&index)
                {
                    let full_json = json_fragments.concat();
                    let input: serde_json::Value =
                        serde_json::from_str(&full_json).unwrap_or(serde_json::json!({}));
                    events.push(ChatEvent::ToolUse { id, name, input });
                }
            }

            ClaudeStreamEvent::MessageDelta { delta } => {
                let stop_reason = delta.stop_reason.unwrap_or_else(|| "end_turn".to_string());
                events.push(ChatEvent::Done { stop_reason });
            }

            ClaudeStreamEvent::Other => {}
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn make_sse(event_type: &str, data: &serde_json::Value) -> Vec<u8> {
        format!(
            "event: {event_type}\ndata: {}\n\n",
            serde_json::to_string(data).unwrap()
        )
        .into_bytes()
    }

    #[test]
    fn text_deltas_produce_text_events() {
        let mut parser = StreamParser::new();

        let chunk = [
            make_sse(
                "content_block_start",
                &serde_json::json!({"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}}),
            ),
            make_sse(
                "content_block_delta",
                &serde_json::json!({"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Hello"}}),
            ),
            make_sse(
                "content_block_delta",
                &serde_json::json!({"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":" world"}}),
            ),
        ]
        .concat();

        let events = parser.feed(&chunk);
        assert_eq!(events.len(), 2);
        match &events[0] {
            ChatEvent::Text { text } => assert_eq!(text, "Hello"),
            other => panic!("expected Text, got {other:?}"),
        }
        match &events[1] {
            ChatEvent::Text { text } => assert_eq!(text, " world"),
            other => panic!("expected Text, got {other:?}"),
        }
    }

    #[test]
    fn tool_use_emitted_on_content_block_stop() {
        let mut parser = StreamParser::new();

        let chunk = [
            make_sse(
                "content_block_start",
                &serde_json::json!({"type":"content_block_start","index":0,"content_block":{"type":"tool_use","id":"tool_1","name":"set_fill"}}),
            ),
            make_sse(
                "content_block_delta",
                &serde_json::json!({"type":"content_block_delta","index":0,"delta":{"type":"input_json_delta","partial_json":"{\"r\":"}}),
            ),
            make_sse(
                "content_block_delta",
                &serde_json::json!({"type":"content_block_delta","index":0,"delta":{"type":"input_json_delta","partial_json":"1.0}"}}),
            ),
            make_sse(
                "content_block_stop",
                &serde_json::json!({"type":"content_block_stop","index":0}),
            ),
        ]
        .concat();

        let events = parser.feed(&chunk);
        assert_eq!(events.len(), 1);
        match &events[0] {
            ChatEvent::ToolUse { id, name, input } => {
                assert_eq!(id, "tool_1");
                assert_eq!(name, "set_fill");
                assert_eq!(input, &serde_json::json!({"r": 1.0}));
            }
            other => panic!("expected ToolUse, got {other:?}"),
        }
    }

    #[test]
    fn mixed_text_and_tool_use() {
        let mut parser = StreamParser::new();

        let chunk = [
            make_sse(
                "content_block_start",
                &serde_json::json!({"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}}),
            ),
            make_sse(
                "content_block_delta",
                &serde_json::json!({"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Setting fill."}}),
            ),
            make_sse(
                "content_block_stop",
                &serde_json::json!({"type":"content_block_stop","index":0}),
            ),
            make_sse(
                "content_block_start",
                &serde_json::json!({"type":"content_block_start","index":1,"content_block":{"type":"tool_use","id":"t2","name":"set_fill"}}),
            ),
            make_sse(
                "content_block_delta",
                &serde_json::json!({"type":"content_block_delta","index":1,"delta":{"type":"input_json_delta","partial_json":"{}"}}),
            ),
            make_sse(
                "content_block_stop",
                &serde_json::json!({"type":"content_block_stop","index":1}),
            ),
        ]
        .concat();

        let events = parser.feed(&chunk);
        assert_eq!(events.len(), 2);
        assert!(matches!(&events[0], ChatEvent::Text { text } if text == "Setting fill."));
        assert!(matches!(&events[1], ChatEvent::ToolUse { name, .. } if name == "set_fill"));
    }

    #[test]
    fn partial_line_split_across_feeds() {
        let mut parser = StreamParser::new();
        let full = format!(
            "data: {}\n\n",
            serde_json::to_string(&serde_json::json!({"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"split"}})).unwrap()
        );

        // Register block first.
        let start = make_sse(
            "content_block_start",
            &serde_json::json!({"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}}),
        );
        parser.feed(&start);

        // Split the data line in the middle.
        let mid = full.len() / 2;
        let part1 = &full.as_bytes()[..mid];
        let part2 = &full.as_bytes()[mid..];

        let events1 = parser.feed(part1);
        assert!(events1.is_empty(), "should not emit from partial line");

        let events2 = parser.feed(part2);
        assert_eq!(events2.len(), 1);
        match &events2[0] {
            ChatEvent::Text { text } => assert_eq!(text, "split"),
            other => panic!("expected Text, got {other:?}"),
        }
    }

    #[test]
    fn malformed_json_skipped() {
        let mut parser = StreamParser::new();
        let chunk = b"data: {not valid json}\n\n";
        let events = parser.feed(chunk);
        assert!(events.is_empty());
    }

    #[test]
    fn empty_text_delta_emits_event() {
        let mut parser = StreamParser::new();

        let chunk = [
            make_sse(
                "content_block_start",
                &serde_json::json!({"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}}),
            ),
            make_sse(
                "content_block_delta",
                &serde_json::json!({"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":""}}),
            ),
        ]
        .concat();

        let events = parser.feed(&chunk);
        assert_eq!(events.len(), 1);
        match &events[0] {
            ChatEvent::Text { text } => assert_eq!(text, ""),
            other => panic!("expected Text, got {other:?}"),
        }
    }

    #[test]
    fn message_delta_null_stop_reason_defaults_to_end_turn() {
        let mut parser = StreamParser::new();
        let chunk = make_sse(
            "message_delta",
            &serde_json::json!({"type":"message_delta","delta":{"stop_reason":null}}),
        );

        let events = parser.feed(&chunk);
        assert_eq!(events.len(), 1);
        match &events[0] {
            ChatEvent::Done { stop_reason } => assert_eq!(stop_reason, "end_turn"),
            other => panic!("expected Done, got {other:?}"),
        }
    }

    #[test]
    fn ping_and_unknown_events_ignored() {
        let mut parser = StreamParser::new();
        let chunk = [
            make_sse(
                "ping",
                &serde_json::json!({"type":"ping"}),
            ),
            make_sse(
                "message_start",
                &serde_json::json!({"type":"message_start","message":{"id":"msg_1","role":"assistant"}}),
            ),
            make_sse(
                "message_stop",
                &serde_json::json!({"type":"message_stop"}),
            ),
        ]
        .concat();

        let events = parser.feed(&chunk);
        assert!(events.is_empty());
    }

    #[test]
    fn multiple_tool_use_blocks() {
        let mut parser = StreamParser::new();

        let chunk = [
            make_sse(
                "content_block_start",
                &serde_json::json!({"type":"content_block_start","index":0,"content_block":{"type":"tool_use","id":"t1","name":"set_fill"}}),
            ),
            make_sse(
                "content_block_delta",
                &serde_json::json!({"type":"content_block_delta","index":0,"delta":{"type":"input_json_delta","partial_json":"{\"r\":1.0}"}}),
            ),
            make_sse(
                "content_block_stop",
                &serde_json::json!({"type":"content_block_stop","index":0}),
            ),
            make_sse(
                "content_block_start",
                &serde_json::json!({"type":"content_block_start","index":1,"content_block":{"type":"tool_use","id":"t2","name":"set_stroke"}}),
            ),
            make_sse(
                "content_block_delta",
                &serde_json::json!({"type":"content_block_delta","index":1,"delta":{"type":"input_json_delta","partial_json":"{\"g\":0.5}"}}),
            ),
            make_sse(
                "content_block_stop",
                &serde_json::json!({"type":"content_block_stop","index":1}),
            ),
        ]
        .concat();

        let events = parser.feed(&chunk);
        assert_eq!(events.len(), 2);
        match &events[0] {
            ChatEvent::ToolUse { id, name, .. } => {
                assert_eq!(id, "t1");
                assert_eq!(name, "set_fill");
            }
            other => panic!("expected ToolUse, got {other:?}"),
        }
        match &events[1] {
            ChatEvent::ToolUse { id, name, .. } => {
                assert_eq!(id, "t2");
                assert_eq!(name, "set_stroke");
            }
            other => panic!("expected ToolUse, got {other:?}"),
        }
    }

    #[test]
    fn tool_use_invalid_json_falls_back_to_empty_object() {
        let mut parser = StreamParser::new();

        let chunk = [
            make_sse(
                "content_block_start",
                &serde_json::json!({"type":"content_block_start","index":0,"content_block":{"type":"tool_use","id":"t1","name":"get_scene_summary"}}),
            ),
            make_sse(
                "content_block_delta",
                &serde_json::json!({"type":"content_block_delta","index":0,"delta":{"type":"input_json_delta","partial_json":"{broken"}}),
            ),
            make_sse(
                "content_block_stop",
                &serde_json::json!({"type":"content_block_stop","index":0}),
            ),
        ]
        .concat();

        let events = parser.feed(&chunk);
        assert_eq!(events.len(), 1);
        match &events[0] {
            ChatEvent::ToolUse { input, .. } => {
                assert_eq!(input, &serde_json::json!({}));
            }
            other => panic!("expected ToolUse, got {other:?}"),
        }
    }
}
