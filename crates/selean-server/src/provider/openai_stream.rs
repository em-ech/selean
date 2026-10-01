//! Stateful SSE parser for chat completions streaming responses.
//!
//! Converts raw byte chunks from an OpenAI-compatible server into
//! [`ChatEvent`] values. Decoupled from async for testability: callers feed
//! chunks via [`EventParser::feed`] and receive parsed events synchronously.

use serde::Deserialize;
use serde_json::Value;

use super::openai_compatible::{WireToolCall, error_message, stop_reason, tool_use_event};
use super::sse::{EventParser, SseDataLines};
use crate::chat::ChatEvent;

/// Sentinel that ends a chat completions stream.
const DONE_SENTINEL: &str = "[DONE]";

#[derive(Debug, Deserialize)]
struct Chunk {
    #[serde(default)]
    choices: Vec<ChunkChoice>,
    error: Option<Value>,
}

#[derive(Debug, Deserialize)]
struct ChunkChoice {
    delta: Option<Delta>,
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Delta {
    content: Option<String>,
    tool_calls: Option<Vec<WireToolCall>>,
}

/// A tool call whose name and arguments are still arriving.
#[derive(Debug, Default)]
struct PendingToolCall {
    index: Option<usize>,
    id: String,
    name: String,
    arguments: String,
}

/// Synchronous SSE parser for the chat completions streaming protocol.
///
/// Text deltas are emitted as they arrive. Tool calls arrive in fragments,
/// so they are accumulated and emitted, followed by [`ChatEvent::Done`],
/// when the stream ends: at the `[DONE]` sentinel, or at the end of the body
/// for servers that omit it.
#[derive(Debug, Default)]
pub struct OpenAiStreamParser {
    lines: SseDataLines,
    pending_tool_calls: Vec<PendingToolCall>,
    finish_reason: Option<String>,
    finished: bool,
}

impl OpenAiStreamParser {
    /// Creates a new parser with empty state.
    pub fn new() -> Self {
        Self::default()
    }

    fn process_data(&mut self, data: &str, events: &mut Vec<ChatEvent>) {
        if self.finished {
            return;
        }
        if data == DONE_SENTINEL {
            self.complete(events);
            return;
        }
        let Ok(chunk) = serde_json::from_str::<Chunk>(data) else {
            return;
        };

        if let Some(error) = &chunk.error {
            events.push(ChatEvent::Error {
                message: error_message(error),
            });
            self.finished = true;
            return;
        }

        // Only one completion is requested, so only the first choice matters.
        let Some(choice) = chunk.choices.into_iter().next() else {
            return;
        };
        if let Some(delta) = choice.delta {
            if let Some(text) = delta.content.filter(|text| !text.is_empty()) {
                events.push(ChatEvent::Text { text });
            }
            for fragment in delta.tool_calls.unwrap_or_default() {
                self.accumulate(fragment);
            }
        }
        if let Some(reason) = choice.finish_reason.filter(|reason| !reason.is_empty()) {
            self.finish_reason = Some(reason);
        }
    }

    /// Merges one streamed fragment into the tool call it belongs to.
    fn accumulate(&mut self, fragment: WireToolCall) {
        let fragment_id = fragment.id.unwrap_or_default();
        let call = self.pending_call_for(fragment.index, &fragment_id);

        if call.id.is_empty() {
            call.id = fragment_id;
        }
        let Some(function) = fragment.function else {
            return;
        };
        if let Some(name) = function.name {
            call.name.push_str(&name);
        }
        match function.arguments {
            Some(Value::String(json)) => call.arguments.push_str(&json),
            Some(Value::Null) | None => {}
            // Some servers send the whole arguments object in one fragment.
            Some(other) => call.arguments.push_str(&other.to_string()),
        }
    }

    /// Finds the pending call a fragment continues, or starts a new one.
    ///
    /// Fragments are matched by `index`. A fragment that carries an ID
    /// different from the one already stored at its index starts a new call:
    /// some servers number every call 0 and distinguish them only by ID. A
    /// fragment with neither index nor ID continues the latest call.
    fn pending_call_for(&mut self, index: Option<usize>, id: &str) -> &mut PendingToolCall {
        let position = match index {
            Some(index) => self.pending_tool_calls.iter().rposition(|call| {
                call.index == Some(index) && (id.is_empty() || call.id.is_empty() || call.id == id)
            }),
            None if id.is_empty() => self.pending_tool_calls.len().checked_sub(1),
            None => self
                .pending_tool_calls
                .iter()
                .rposition(|call| call.id == id),
        };

        let position = position.unwrap_or_else(|| {
            self.pending_tool_calls.push(PendingToolCall {
                index,
                ..PendingToolCall::default()
            });
            self.pending_tool_calls.len() - 1
        });
        &mut self.pending_tool_calls[position]
    }

    /// Emits the accumulated tool calls and the closing `Done` event.
    fn complete(&mut self, events: &mut Vec<ChatEvent>) {
        self.finished = true;

        let tool_events: Vec<ChatEvent> = self
            .pending_tool_calls
            .drain(..)
            .filter_map(|call| tool_use_event(Some(call.id), call.name, &call.arguments))
            .collect();
        let made_tool_calls = !tool_events.is_empty();
        events.extend(tool_events);

        events.push(ChatEvent::Done {
            stop_reason: stop_reason(self.finish_reason.as_deref(), made_tool_calls),
        });
    }
}

impl EventParser for OpenAiStreamParser {
    fn feed(&mut self, chunk: &[u8]) -> Vec<ChatEvent> {
        let mut events = Vec::new();
        for data in self.lines.feed(chunk) {
            self.process_data(&data, &mut events);
        }
        events
    }

    /// Completes the turn if the body ended without the `[DONE]` sentinel.
    fn finish(&mut self) -> Vec<ChatEvent> {
        let mut events = Vec::new();
        if let Some(data) = self.lines.finish() {
            self.process_data(&data, &mut events);
        }
        if !self.finished {
            self.complete(&mut events);
        }
        events
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use serde_json::json;

    use super::*;

    /// Feeds a whole recorded stream in one chunk and ends the body.
    fn parse(stream: &str) -> Vec<ChatEvent> {
        let mut parser = OpenAiStreamParser::new();
        let mut events = parser.feed(stream.as_bytes());
        events.extend(parser.finish());
        events
    }

    fn text_of(event: &ChatEvent) -> &str {
        match event {
            ChatEvent::Text { text } => text,
            other => panic!("expected Text, got {other:?}"),
        }
    }

    fn tool_use_of(event: &ChatEvent) -> (&str, &str, &Value) {
        match event {
            ChatEvent::ToolUse { id, name, input } => (id, name, input),
            other => panic!("expected ToolUse, got {other:?}"),
        }
    }

    fn stop_reason_of(event: &ChatEvent) -> &str {
        match event {
            ChatEvent::Done { stop_reason } => stop_reason,
            other => panic!("expected Done, got {other:?}"),
        }
    }

    const TEXT_STREAM: &str = r#"data: {"id":"chatcmpl-1","object":"chat.completion.chunk","choices":[{"index":0,"delta":{"role":"assistant","content":""},"finish_reason":null}]}

data: {"id":"chatcmpl-1","object":"chat.completion.chunk","choices":[{"index":0,"delta":{"content":"Hello"},"finish_reason":null}]}

data: {"id":"chatcmpl-1","object":"chat.completion.chunk","choices":[{"index":0,"delta":{"content":" world"},"finish_reason":null}]}

data: {"id":"chatcmpl-1","object":"chat.completion.chunk","choices":[{"index":0,"delta":{},"finish_reason":"stop"}]}

data: [DONE]

"#;

    /// One tool call whose arguments arrive in four fragments.
    const SPLIT_TOOL_CALL_STREAM: &str = r#"data: {"choices":[{"index":0,"delta":{"role":"assistant","content":null,"tool_calls":[{"index":0,"id":"call_abc","type":"function","function":{"name":"set_fill","arguments":""}}]},"finish_reason":null}]}

data: {"choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"function":{"arguments":"{\"node_id\":"}}]},"finish_reason":null}]}

data: {"choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"function":{"arguments":"\"abc\",\"r\""}}]},"finish_reason":null}]}

data: {"choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"function":{"arguments":":1.0}"}}]},"finish_reason":null}]}

data: {"choices":[{"index":0,"delta":{},"finish_reason":"tool_calls"}]}

data: [DONE]

"#;

    /// Text, then two tool calls whose fragments are interleaved by index.
    const TWO_TOOL_CALLS_STREAM: &str = r#"data: {"choices":[{"index":0,"delta":{"content":"On it."},"finish_reason":null}]}

data: {"choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"id":"call_1","type":"function","function":{"name":"set_fill","arguments":"{\"r\":"}}]},"finish_reason":null}]}

data: {"choices":[{"index":0,"delta":{"tool_calls":[{"index":1,"id":"call_2","type":"function","function":{"name":"set_stroke","arguments":""}}]},"finish_reason":null}]}

data: {"choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"function":{"arguments":"1.0}"}},{"index":1,"function":{"arguments":"{\"g\":0.5}"}}]},"finish_reason":null}]}

data: {"choices":[{"index":0,"delta":{},"finish_reason":"tool_calls"}]}

data: {"choices":[],"usage":{"prompt_tokens":10,"completion_tokens":20,"total_tokens":30}}

data: [DONE]

"#;

    #[test]
    fn text_deltas_produce_text_events_then_done() {
        let events = parse(TEXT_STREAM);
        assert_eq!(events.len(), 3);
        assert_eq!(text_of(&events[0]), "Hello");
        assert_eq!(text_of(&events[1]), " world");
        assert_eq!(stop_reason_of(&events[2]), "end_turn");
    }

    #[test]
    fn text_is_emitted_as_it_arrives_and_done_waits_for_the_sentinel() {
        let mut parser = OpenAiStreamParser::new();
        let (before_done, done) = TEXT_STREAM
            .split_once("data: [DONE]")
            .expect("fixture has a sentinel");

        let events = parser.feed(before_done.as_bytes());
        assert_eq!(events.len(), 2);
        assert!(events.iter().all(|e| matches!(e, ChatEvent::Text { .. })));

        let events = parser.feed(format!("data: [DONE]{done}").as_bytes());
        assert_eq!(events.len(), 1);
        assert_eq!(stop_reason_of(&events[0]), "end_turn");
        assert!(parser.finish().is_empty(), "Done must be emitted once");
    }

    #[test]
    fn tool_call_split_across_chunks_is_assembled() {
        let events = parse(SPLIT_TOOL_CALL_STREAM);
        assert_eq!(events.len(), 2);
        let (id, name, input) = tool_use_of(&events[0]);
        assert_eq!(id, "call_abc");
        assert_eq!(name, "set_fill");
        assert_eq!(input, &json!({"node_id": "abc", "r": 1.0}));
        assert_eq!(stop_reason_of(&events[1]), "tool_use");
    }

    #[test]
    fn tool_call_is_assembled_when_the_body_is_fed_byte_by_byte() {
        let mut parser = OpenAiStreamParser::new();
        let mut events = Vec::new();
        for byte in SPLIT_TOOL_CALL_STREAM.as_bytes() {
            events.extend(parser.feed(std::slice::from_ref(byte)));
        }
        events.extend(parser.finish());

        assert_eq!(events.len(), 2);
        let (_, name, input) = tool_use_of(&events[0]);
        assert_eq!(name, "set_fill");
        assert_eq!(input, &json!({"node_id": "abc", "r": 1.0}));
    }

    #[test]
    fn multiple_interleaved_tool_calls_are_emitted_in_order() {
        let events = parse(TWO_TOOL_CALLS_STREAM);
        assert_eq!(events.len(), 4);
        assert_eq!(text_of(&events[0]), "On it.");
        let (id, name, input) = tool_use_of(&events[1]);
        assert_eq!((id, name), ("call_1", "set_fill"));
        assert_eq!(input, &json!({"r": 1.0}));
        let (id, name, input) = tool_use_of(&events[2]);
        assert_eq!((id, name), ("call_2", "set_stroke"));
        assert_eq!(input, &json!({"g": 0.5}));
        assert_eq!(stop_reason_of(&events[3]), "tool_use");
    }

    #[test]
    fn whole_tool_calls_sharing_index_zero_stay_separate() {
        // Some servers send each call complete in one chunk, all numbered 0,
        // and finish the turn with "stop".
        let events = parse(
            r#"data: {"choices":[{"index":0,"delta":{"role":"assistant","content":"","tool_calls":[{"id":"call_a","index":0,"type":"function","function":{"name":"set_fill","arguments":"{\"r\":1.0}"}}]},"finish_reason":null}]}

data: {"choices":[{"index":0,"delta":{"role":"assistant","content":"","tool_calls":[{"id":"call_b","index":0,"type":"function","function":{"name":"undo","arguments":"{}"}}]},"finish_reason":null}]}

data: {"choices":[{"index":0,"delta":{"role":"assistant","content":""},"finish_reason":"stop"}]}

data: [DONE]

"#,
        );
        assert_eq!(events.len(), 3);
        assert_eq!(tool_use_of(&events[0]).0, "call_a");
        assert_eq!(tool_use_of(&events[0]).2, &json!({"r": 1.0}));
        assert_eq!(tool_use_of(&events[1]).0, "call_b");
        assert_eq!(stop_reason_of(&events[2]), "tool_use");
    }

    #[test]
    fn fragments_without_index_or_id_continue_the_latest_call() {
        let events = parse(
            r#"data: {"choices":[{"delta":{"tool_calls":[{"id":"call_a","function":{"name":"set_fill","arguments":"{\"r\""}}]}}]}

data: {"choices":[{"delta":{"tool_calls":[{"function":{"arguments":":1.0}"}}]}}]}

data: [DONE]
"#,
        );
        assert_eq!(events.len(), 2);
        let (id, _, input) = tool_use_of(&events[0]);
        assert_eq!(id, "call_a");
        assert_eq!(input, &json!({"r": 1.0}));
    }

    #[test]
    fn arguments_sent_as_an_object_are_accepted() {
        let events = parse(
            r#"data: {"choices":[{"delta":{"tool_calls":[{"index":0,"id":"call_a","function":{"name":"set_fill","arguments":{"r":1.0}}}]}}]}

data: [DONE]
"#,
        );
        assert_eq!(tool_use_of(&events[0]).2, &json!({"r": 1.0}));
    }

    #[test]
    fn malformed_tool_arguments_fall_back_to_an_empty_object() {
        let events = parse(
            r#"data: {"choices":[{"delta":{"tool_calls":[{"index":0,"id":"call_a","function":{"name":"set_fill","arguments":"{broken"}}]}}]}

data: [DONE]
"#,
        );
        assert_eq!(tool_use_of(&events[0]).2, &json!({}));
        assert_eq!(stop_reason_of(&events[1]), "tool_use");
    }

    #[test]
    fn tool_call_without_an_id_gets_a_generated_one() {
        let events = parse(
            r#"data: {"choices":[{"delta":{"tool_calls":[{"index":0,"function":{"name":"undo","arguments":"{}"}}]}}]}

data: [DONE]
"#,
        );
        let (id, name, _) = tool_use_of(&events[0]);
        assert!(id.starts_with("call_") && id.len() > 5, "got: {id}");
        assert_eq!(name, "undo");
    }

    #[test]
    fn length_finish_reason_maps_to_max_tokens() {
        let events = parse(
            r#"data: {"choices":[{"delta":{"content":"cut"},"finish_reason":"length"}]}

data: [DONE]
"#,
        );
        assert_eq!(stop_reason_of(&events[1]), "max_tokens");
    }

    #[test]
    fn body_ending_without_the_sentinel_still_completes_the_turn() {
        let mut parser = OpenAiStreamParser::new();
        let events = parser.feed(
            br#"data: {"choices":[{"delta":{"tool_calls":[{"index":0,"id":"call_a","function":{"name":"undo","arguments":"{}"}}]},"finish_reason":"tool_calls"}]}
"#,
        );
        assert!(events.is_empty(), "tool calls wait for the end of the turn");

        let events = parser.finish();
        assert_eq!(events.len(), 2);
        assert_eq!(tool_use_of(&events[0]).1, "undo");
        assert_eq!(stop_reason_of(&events[1]), "tool_use");
    }

    #[test]
    fn sentinel_without_a_trailing_newline_is_handled_at_the_end_of_the_body() {
        let mut parser = OpenAiStreamParser::new();
        let events =
            parser.feed(b"data: {\"choices\":[{\"delta\":{\"content\":\"Hi\"}}]}\n\ndata: [DONE]");
        assert_eq!(events.len(), 1);

        let events = parser.finish();
        assert_eq!(events.len(), 1);
        assert_eq!(stop_reason_of(&events[0]), "end_turn");
    }

    #[test]
    fn nothing_is_emitted_after_the_sentinel() {
        let mut parser = OpenAiStreamParser::new();
        let mut events = parser.feed(TEXT_STREAM.as_bytes());
        events.extend(parser.feed(TEXT_STREAM.as_bytes()));
        events.extend(parser.finish());
        assert_eq!(events.len(), 3);
    }

    #[test]
    fn comments_malformed_json_and_empty_choices_are_skipped() {
        let events = parse(
            r#": OPENROUTER PROCESSING

data: {not valid json}

data: {"choices":[]}

data: {"choices":[{"delta":{"content":"ok","reasoning":"thinking"}}]}

data: [DONE]
"#,
        );
        assert_eq!(events.len(), 2);
        assert_eq!(text_of(&events[0]), "ok");
    }

    #[test]
    fn error_chunk_becomes_an_error_event_and_ends_the_turn() {
        let events = parse(
            r#"data: {"choices":[{"delta":{"content":"partial"}}]}

data: {"error":{"message":"Provider disconnected","code":502}}

data: [DONE]
"#,
        );
        assert_eq!(events.len(), 2);
        assert_eq!(text_of(&events[0]), "partial");
        match &events[1] {
            ChatEvent::Error { message } => assert_eq!(message, "Provider disconnected"),
            other => panic!("expected Error, got {other:?}"),
        }
    }
}
