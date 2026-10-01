//! OpenAI-compatible provider: any server that implements the chat
//! completions API, such as a local Ollama server or `OpenRouter`.
//!
//! The web app speaks Claude's message format, so this module translates in
//! both directions: Claude-style messages and tool definitions into a chat
//! completions request, and the completion back into [`ChatEvent`] values.

use selean_llm::{ToolDefinition, all_tools};
use serde::Deserialize;
use serde_json::{Value, json};

use super::openai_stream::OpenAiStreamParser;
use super::sse::EventParser;
use super::{ChatProvider, Failure, MAX_OUTPUT_TOKENS};
use crate::chat::{ChatError, ChatEvent, ChatMessage};
use crate::state::AppState;

/// Base URL of a local Ollama server's OpenAI-compatible API.
pub const DEFAULT_BASE_URL: &str = "http://localhost:11434/v1";

/// Model used when `LLM_MODEL` is not set: tool-capable in Ollama and small
/// enough for a laptop.
pub const DEFAULT_MODEL: &str = "qwen2.5:7b";

/// Port Ollama listens on; used to recognise an Ollama server so that error
/// messages can name the exact `ollama` command to run.
const OLLAMA_PORT_MARKER: &str = ":11434";

/// The OpenAI-compatible provider.
pub struct OpenAiCompatible;

impl ChatProvider for OpenAiCompatible {
    fn request(
        &self,
        state: &AppState,
        system: &str,
        messages: &[ChatMessage],
        stream: bool,
    ) -> Result<reqwest::RequestBuilder, ChatError> {
        let url = format!("{}/chat/completions", base_url(state));
        let body = build_request(&state.model, system, messages, &all_tools(), stream);

        let request = state.http_client.post(url).json(&body);
        // A local server needs no key, so the header is only sent with one.
        Ok(match state.api_key.as_deref() {
            Some(api_key) => request.bearer_auth(api_key),
            None => request,
        })
    }

    fn parse_response(&self, body: &str) -> Result<Vec<ChatEvent>, ChatError> {
        parse_completion(body)
    }

    fn stream_parser(&self) -> Box<dyn EventParser> {
        Box::new(OpenAiStreamParser::new())
    }

    fn explain_failure(&self, state: &AppState, failure: &Failure) -> Option<String> {
        explain_failure(
            base_url(state),
            &state.model,
            state.api_key.is_some(),
            failure,
        )
    }
}

fn base_url(state: &AppState) -> &str {
    state.llm_base_url.as_deref().unwrap_or(DEFAULT_BASE_URL)
}

// ── Request translation ─────────────────────────────────────────────

/// Builds the chat completions request body.
pub fn build_request(
    model: &str,
    system: &str,
    messages: &[ChatMessage],
    tools: &[ToolDefinition],
    stream: bool,
) -> Value {
    let mut body = json!({
        "model": model,
        "max_tokens": MAX_OUTPUT_TOKENS,
        "messages": translate_messages(system, messages),
        "tools": tools.iter().map(translate_tool).collect::<Vec<_>>(),
    });
    if stream {
        body["stream"] = json!(true);
    }
    body
}

/// Translates a Claude-style tool definition into a chat completions
/// function tool: `input_schema` becomes `function.parameters`.
pub fn translate_tool(tool: &ToolDefinition) -> Value {
    json!({
        "type": "function",
        "function": {
            "name": tool.name,
            "description": tool.description,
            "parameters": tool.input_schema,
        },
    })
}

/// Translates the system prompt and the Claude-style conversation into chat
/// completions messages.
pub fn translate_messages(system: &str, messages: &[ChatMessage]) -> Vec<Value> {
    let mut translated = vec![json!({"role": "system", "content": system})];
    for message in messages {
        if message.role == "assistant" {
            translated.push(translate_assistant_message(&message.content));
        } else {
            translated.extend(translate_user_message(&message.content));
        }
    }
    translated
}

/// An assistant turn becomes one message: its text blocks joined into
/// `content` and its `tool_use` blocks listed as `tool_calls`.
fn translate_assistant_message(content: &Value) -> Value {
    let tool_calls: Vec<Value> = blocks_of_type(content, "tool_use")
        .map(|block| {
            json!({
                "id": block["id"],
                "type": "function",
                "function": {
                    "name": block["name"],
                    // Chat completions carries arguments as a JSON string.
                    "arguments": block.get("input").unwrap_or(&json!({})).to_string(),
                },
            })
        })
        .collect();

    let text = text_of(content);
    if tool_calls.is_empty() {
        return json!({"role": "assistant", "content": text});
    }
    let content = if text.is_empty() {
        Value::Null
    } else {
        Value::String(text)
    };
    json!({"role": "assistant", "content": content, "tool_calls": tool_calls})
}

/// A user turn becomes one `tool` message per `tool_result` block, followed
/// by a `user` message for any text. Tool messages come first because they
/// must directly follow the assistant message that made the calls.
fn translate_user_message(content: &Value) -> Vec<Value> {
    let mut translated: Vec<Value> = blocks_of_type(content, "tool_result")
        .map(|block| {
            json!({
                "role": "tool",
                "tool_call_id": block["tool_use_id"],
                "content": text_of(block.get("content").unwrap_or(&Value::Null)),
            })
        })
        .collect();

    let text = text_of(content);
    if !text.is_empty() || translated.is_empty() {
        translated.push(json!({"role": "user", "content": text}));
    }
    translated
}

/// Iterates over the content blocks of the given type. Plain-string content
/// has no blocks.
fn blocks_of_type<'a>(content: &'a Value, block_type: &'a str) -> impl Iterator<Item = &'a Value> {
    content
        .as_array()
        .into_iter()
        .flatten()
        .filter(move |block| block["type"] == block_type)
}

/// Returns the text of Claude-style content: the string itself, or the text
/// blocks of a block array joined together.
fn text_of(content: &Value) -> String {
    match content {
        Value::String(text) => text.clone(),
        Value::Null => String::new(),
        Value::Array(_) => blocks_of_type(content, "text")
            .filter_map(|block| block["text"].as_str())
            .collect(),
        other => other.to_string(),
    }
}

// ── Response translation ────────────────────────────────────────────

/// A tool call as it appears in a completion message or a streaming delta.
/// Every field is optional because streaming sends them piecemeal and
/// servers differ in what they include.
#[derive(Debug, Deserialize)]
pub(super) struct WireToolCall {
    pub(super) index: Option<usize>,
    pub(super) id: Option<String>,
    pub(super) function: Option<WireFunction>,
}

/// The function part of a [`WireToolCall`].
#[derive(Debug, Deserialize)]
pub(super) struct WireFunction {
    pub(super) name: Option<String>,
    /// A JSON string per the API; some servers send the object itself.
    pub(super) arguments: Option<Value>,
}

#[derive(Debug, Deserialize)]
struct Completion {
    #[serde(default)]
    choices: Vec<Choice>,
    error: Option<Value>,
}

#[derive(Debug, Deserialize)]
struct Choice {
    message: CompletionMessage,
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CompletionMessage {
    content: Option<String>,
    tool_calls: Option<Vec<WireToolCall>>,
}

/// Translates a non-streaming chat completions body into chat events.
fn parse_completion(body: &str) -> Result<Vec<ChatEvent>, ChatError> {
    let completion: Completion =
        serde_json::from_str(body).map_err(|e| ChatError::ParseError(e.to_string()))?;

    // Some hosts report failures in a 200 response.
    if let Some(error) = &completion.error {
        return Err(ChatError::Provider(error_message(error)));
    }
    let Some(choice) = completion.choices.into_iter().next() else {
        return Err(ChatError::ParseError(
            "the response has no choices".to_string(),
        ));
    };

    let mut events = Vec::new();
    if let Some(text) = choice.message.content.filter(|text| !text.is_empty()) {
        events.push(ChatEvent::Text { text });
    }

    let tool_events: Vec<ChatEvent> = choice
        .message
        .tool_calls
        .unwrap_or_default()
        .into_iter()
        .filter_map(|call| {
            let function = call.function?;
            let arguments = match function.arguments {
                Some(Value::String(json)) => json,
                Some(other) => other.to_string(),
                None => String::new(),
            };
            tool_use_event(call.id, function.name.unwrap_or_default(), &arguments)
        })
        .collect();
    let made_tool_calls = !tool_events.is_empty();
    events.extend(tool_events);

    events.push(ChatEvent::Done {
        stop_reason: stop_reason(choice.finish_reason.as_deref(), made_tool_calls),
    });
    Ok(events)
}

/// Builds the [`ChatEvent::ToolUse`] for a finished tool call, or `None` if
/// the model did not name a tool. Servers that omit the call ID get a
/// generated one, since the client pairs each tool result with its call by ID.
pub(super) fn tool_use_event(
    id: Option<String>,
    name: String,
    arguments: &str,
) -> Option<ChatEvent> {
    if name.is_empty() {
        return None;
    }
    let id = id
        .filter(|id| !id.is_empty())
        .unwrap_or_else(|| format!("call_{}", uuid::Uuid::new_v4().simple()));
    Some(ChatEvent::ToolUse {
        id,
        name,
        input: parse_arguments(arguments),
    })
}

/// Parses a tool call's arguments into the JSON object the client expects.
/// Anything that is not an object, including malformed JSON from a weak
/// model, becomes an empty object, as it does on the Claude path.
fn parse_arguments(arguments: &str) -> Value {
    match serde_json::from_str::<Value>(arguments) {
        Ok(object @ Value::Object(_)) => object,
        // Some models encode the arguments twice.
        Ok(Value::String(inner)) => match serde_json::from_str::<Value>(&inner) {
            Ok(object @ Value::Object(_)) => object,
            _ => json!({}),
        },
        _ => json!({}),
    }
}

/// Maps a chat completions finish reason onto the Claude stop reason the
/// client expects.
///
/// The client runs tools only on `tool_use`, so that is reported exactly
/// when tool calls were emitted: some servers finish a tool-calling turn
/// with `stop`, and a `tool_calls` finish with no usable call must not send
/// the client into a tool round with nothing to run.
pub(super) fn stop_reason(finish_reason: Option<&str>, made_tool_calls: bool) -> String {
    let reason = match finish_reason {
        _ if made_tool_calls => "tool_use",
        Some("length") => "max_tokens",
        Some("content_filter") => "refusal",
        _ => "end_turn",
    };
    reason.to_string()
}

/// Extracts a readable message from a chat completions `error` value.
pub(super) fn error_message(error: &Value) -> String {
    error
        .get("message")
        .and_then(Value::as_str)
        .map_or_else(|| error.to_string(), str::to_string)
}

// ── Actionable failure messages ─────────────────────────────────────

/// Explains the failures a user can fix themselves: the server is not
/// running, the model is not downloaded or cannot call tools, or the key is
/// wrong. Returns `None` for anything else.
fn explain_failure(
    base_url: &str,
    model: &str,
    has_api_key: bool,
    failure: &Failure,
) -> Option<String> {
    let is_ollama = base_url.contains(OLLAMA_PORT_MARKER);

    match failure {
        Failure::Unreachable => Some(if is_ollama {
            format!(
                "AI chat could not reach Ollama at {base_url}. Start it (open the Ollama app or \
                 run `ollama serve`), make sure the model is downloaded (`ollama pull {model}`), \
                 then try again."
            )
        } else {
            format!(
                "AI chat could not reach the model server at {base_url}. Check LLM_BASE_URL and \
                 that the server is running."
            )
        }),
        Failure::Status { status: 404, body } if body.contains("model") => Some(if is_ollama {
            format!(
                "The model `{model}` is not downloaded. Run `ollama pull {model}` and try again."
            )
        } else {
            format!("The model `{model}` was not found at {base_url}. Check LLM_MODEL.")
        }),
        Failure::Status { status: 404, .. } => Some(format!(
            "No chat completions API was found at {base_url}. Check LLM_BASE_URL; it must end \
             with the API root (for Ollama, `/v1`)."
        )),
        Failure::Status { status: 400, body } if body.contains("does not support tools") => {
            Some(format!(
                "The model `{model}` does not support tool calling, which AI chat needs. Set \
                 LLM_MODEL to a model that does."
            ))
        }
        Failure::Status {
            status: 401 | 403, ..
        } => Some(if has_api_key {
            format!("The model server at {base_url} rejected the API key. Check LLM_API_KEY.")
        } else {
            format!("The model server at {base_url} requires an API key. Set LLM_API_KEY.")
        }),
        Failure::Status { .. } => None,
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use std::sync::Arc;

    use super::*;

    fn message(role: &str, content: Value) -> ChatMessage {
        ChatMessage {
            role: role.to_string(),
            content,
        }
    }

    // ── Request translation ─────────────────────────────────────────

    #[test]
    fn tool_definition_becomes_a_function_tool_with_parameters() {
        let tool: ToolDefinition = serde_json::from_value(json!({
            "name": "set_opacity",
            "description": "Set a node's opacity.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "node_id": {"type": "string", "description": "UUID of the node"},
                    "opacity": {"type": "number", "description": "Opacity from 0.0 to 1.0"},
                    "mode": {"type": "string", "description": "Blend", "enum": ["a", "b"]},
                },
                "required": ["node_id", "opacity"],
            },
        }))
        .expect("valid tool definition");

        let translated = translate_tool(&tool);

        assert_eq!(translated["type"], "function");
        assert_eq!(translated["function"]["name"], "set_opacity");
        assert_eq!(
            translated["function"]["description"],
            "Set a node's opacity."
        );
        let parameters = &translated["function"]["parameters"];
        assert_eq!(parameters["type"], "object");
        assert_eq!(parameters["properties"]["node_id"]["type"], "string");
        assert_eq!(parameters["properties"]["opacity"]["type"], "number");
        assert_eq!(parameters["properties"]["mode"]["enum"], json!(["a", "b"]));
        assert_eq!(parameters["required"], json!(["node_id", "opacity"]));
        assert!(translated.get("input_schema").is_none());
        assert!(translated["function"].get("input_schema").is_none());
    }

    #[test]
    fn build_request_carries_model_limit_system_message_and_every_tool() {
        let tools = all_tools();
        let body = build_request(
            "qwen2.5:7b",
            "be helpful",
            &[message("user", json!("Make it red"))],
            &tools,
            false,
        );

        assert_eq!(body["model"], "qwen2.5:7b");
        assert_eq!(body["max_tokens"], MAX_OUTPUT_TOKENS);
        assert_eq!(
            body["messages"],
            json!([
                {"role": "system", "content": "be helpful"},
                {"role": "user", "content": "Make it red"},
            ])
        );
        let sent_tools = body["tools"].as_array().unwrap();
        assert_eq!(sent_tools.len(), tools.len());
        assert!(sent_tools.iter().all(|tool| tool["type"] == "function"));
        assert!(body.get("stream").is_none());
        assert!(body.get("system").is_none());
    }

    #[test]
    fn build_request_sets_stream_flag_when_streaming() {
        let body = build_request("m", "", &[], &[], true);
        assert_eq!(body["stream"], true);
    }

    #[test]
    fn assistant_text_blocks_are_joined_into_string_content() {
        let translated = translate_messages(
            "sys",
            &[message(
                "assistant",
                json!([{"type": "text", "text": "Hello "}, {"type": "text", "text": "there"}]),
            )],
        );
        assert_eq!(
            translated[1],
            json!({"role": "assistant", "content": "Hello there"})
        );
    }

    #[test]
    fn assistant_tool_use_becomes_tool_calls_with_string_arguments() {
        let translated = translate_messages(
            "sys",
            &[message(
                "assistant",
                json!([
                    {"type": "text", "text": "Setting the fill."},
                    {"type": "tool_use", "id": "call_1", "name": "set_fill",
                     "input": {"node_id": "abc", "r": 1.0}},
                    {"type": "tool_use", "id": "call_2", "name": "get_scene_summary", "input": {}},
                ]),
            )],
        );

        let assistant = &translated[1];
        assert_eq!(assistant["role"], "assistant");
        assert_eq!(assistant["content"], "Setting the fill.");
        let calls = assistant["tool_calls"].as_array().unwrap();
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0]["id"], "call_1");
        assert_eq!(calls[0]["type"], "function");
        assert_eq!(calls[0]["function"]["name"], "set_fill");
        let arguments: Value =
            serde_json::from_str(calls[0]["function"]["arguments"].as_str().unwrap()).unwrap();
        assert_eq!(arguments, json!({"node_id": "abc", "r": 1.0}));
        assert_eq!(calls[1]["function"]["arguments"], "{}");
    }

    #[test]
    fn assistant_with_only_tool_use_has_null_content() {
        let translated = translate_messages(
            "sys",
            &[message(
                "assistant",
                json!([{"type": "tool_use", "id": "call_1", "name": "undo", "input": {}}]),
            )],
        );
        assert!(translated[1]["content"].is_null());
        assert_eq!(translated[1]["tool_calls"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn tool_results_become_one_tool_message_each() {
        let translated = translate_messages(
            "sys",
            &[message(
                "user",
                json!([
                    {"type": "tool_result", "tool_use_id": "call_1", "content": "{\"ok\":true}"},
                    {"type": "tool_result", "tool_use_id": "call_2",
                     "content": [{"type": "text", "text": "done"}]},
                ]),
            )],
        );

        assert_eq!(
            translated[1..],
            [
                json!({"role": "tool", "tool_call_id": "call_1", "content": "{\"ok\":true}"}),
                json!({"role": "tool", "tool_call_id": "call_2", "content": "done"}),
            ]
        );
    }

    #[test]
    fn user_text_alongside_tool_results_follows_the_tool_messages() {
        let translated = translate_messages(
            "sys",
            &[message(
                "user",
                json!([
                    {"type": "text", "text": "Now make it blue"},
                    {"type": "tool_result", "tool_use_id": "call_1", "content": "ok"},
                ]),
            )],
        );

        assert_eq!(
            translated[1..],
            [
                json!({"role": "tool", "tool_call_id": "call_1", "content": "ok"}),
                json!({"role": "user", "content": "Now make it blue"}),
            ]
        );
    }

    #[test]
    fn tool_result_without_content_has_empty_string_content() {
        let translated = translate_messages(
            "sys",
            &[message(
                "user",
                json!([{"type": "tool_result", "tool_use_id": "call_1"}]),
            )],
        );
        assert_eq!(
            translated[1],
            json!({"role": "tool", "tool_call_id": "call_1", "content": ""})
        );
    }

    #[test]
    fn full_tool_round_trip_keeps_conversation_order() {
        let translated = translate_messages(
            "sys",
            &[
                message("user", json!("Make it red")),
                message(
                    "assistant",
                    json!([{"type": "tool_use", "id": "call_1", "name": "set_fill", "input": {"r": 1.0}}]),
                ),
                message(
                    "user",
                    json!([{"type": "tool_result", "tool_use_id": "call_1", "content": "ok"}]),
                ),
                message("assistant", json!([{"type": "text", "text": "Done."}])),
            ],
        );

        let roles: Vec<&str> = translated
            .iter()
            .map(|m| m["role"].as_str().unwrap())
            .collect();
        assert_eq!(roles, ["system", "user", "assistant", "tool", "assistant"]);
    }

    // ── Request building ────────────────────────────────────────────

    #[test]
    fn request_without_api_key_sends_no_authorization_header() {
        let mut state = AppState::new_test();
        state.llm_base_url = Some(Arc::from("http://localhost:11434/v1"));
        state.api_key = None;

        let request = OpenAiCompatible
            .request(&state, "sys", &[message("user", json!("hi"))], true)
            .expect("no key is needed")
            .build()
            .expect("valid request");

        assert_eq!(
            request.url().as_str(),
            "http://localhost:11434/v1/chat/completions"
        );
        assert!(request.headers().get("authorization").is_none());
        assert!(request.headers().get("x-api-key").is_none());
    }

    #[test]
    fn request_with_api_key_sends_it_as_a_bearer_token() {
        let mut state = AppState::new_test();
        state.llm_base_url = Some(Arc::from("https://openrouter.ai/api/v1"));
        state.api_key = Some(Arc::from("test-key"));

        let request = OpenAiCompatible
            .request(&state, "sys", &[], false)
            .expect("configured")
            .build()
            .expect("valid request");

        assert_eq!(
            request.url().as_str(),
            "https://openrouter.ai/api/v1/chat/completions"
        );
        assert_eq!(request.headers()["authorization"], "Bearer test-key");
    }

    // ── Response translation ────────────────────────────────────────

    #[test]
    fn text_completion_maps_to_text_and_end_turn() {
        let body = r#"{"id":"chatcmpl-1","object":"chat.completion","choices":[{"index":0,
            "message":{"role":"assistant","content":"I made it red."},"finish_reason":"stop"}],
            "usage":{"prompt_tokens":10,"completion_tokens":5,"total_tokens":15}}"#;

        let events = parse_completion(body).expect("valid body");

        assert_eq!(events.len(), 2);
        assert!(matches!(&events[0], ChatEvent::Text { text } if text == "I made it red."));
        assert!(matches!(&events[1], ChatEvent::Done { stop_reason } if stop_reason == "end_turn"));
    }

    #[test]
    fn tool_call_completion_maps_to_tool_use_events() {
        let body = r#"{"choices":[{"index":0,"message":{"role":"assistant","content":null,
            "tool_calls":[
              {"id":"call_a","type":"function","function":{"name":"set_fill",
               "arguments":"{\"node_id\":\"abc\",\"r\":1.0}"}},
              {"id":"call_b","type":"function","function":{"name":"undo","arguments":"{}"}}
            ]},"finish_reason":"tool_calls"}]}"#;

        let events = parse_completion(body).expect("valid body");

        assert_eq!(events.len(), 3);
        match &events[0] {
            ChatEvent::ToolUse { id, name, input } => {
                assert_eq!(id, "call_a");
                assert_eq!(name, "set_fill");
                assert_eq!(input, &json!({"node_id": "abc", "r": 1.0}));
            }
            other => panic!("expected ToolUse, got {other:?}"),
        }
        assert!(matches!(&events[1], ChatEvent::ToolUse { id, name, .. }
            if id == "call_b" && name == "undo"));
        assert!(matches!(&events[2], ChatEvent::Done { stop_reason } if stop_reason == "tool_use"));
    }

    #[test]
    fn text_before_tool_calls_is_kept() {
        let body = r#"{"choices":[{"message":{"content":"Setting the fill.",
            "tool_calls":[{"id":"call_a","function":{"name":"undo","arguments":"{}"}}]},
            "finish_reason":"tool_calls"}]}"#;

        let events = parse_completion(body).expect("valid body");
        assert!(matches!(&events[0], ChatEvent::Text { text } if text == "Setting the fill."));
        assert!(matches!(&events[1], ChatEvent::ToolUse { .. }));
    }

    #[test]
    fn tool_calls_with_a_stop_finish_reason_still_report_tool_use() {
        let body = r#"{"choices":[{"message":{"content":"",
            "tool_calls":[{"id":"call_a","function":{"name":"undo","arguments":"{}"}}]},
            "finish_reason":"stop"}]}"#;

        let events = parse_completion(body).expect("valid body");
        assert_eq!(events.len(), 2);
        assert!(matches!(&events[1], ChatEvent::Done { stop_reason } if stop_reason == "tool_use"));
    }

    #[test]
    fn tool_call_without_an_id_gets_a_generated_one() {
        let body = r#"{"choices":[{"message":{"content":null,
            "tool_calls":[{"function":{"name":"undo","arguments":{}}},
                          {"id":"","function":{"name":"redo","arguments":"{}"}}]},
            "finish_reason":"tool_calls"}]}"#;

        let events = parse_completion(body).expect("valid body");
        let ids: Vec<&String> = events
            .iter()
            .filter_map(|event| match event {
                ChatEvent::ToolUse { id, .. } => Some(id),
                _ => None,
            })
            .collect();
        assert_eq!(ids.len(), 2);
        assert!(ids.iter().all(|id| id.starts_with("call_") && id.len() > 5));
        assert_ne!(ids[0], ids[1]);
    }

    #[test]
    fn tool_call_without_a_name_is_dropped_and_the_turn_ends() {
        let body = r#"{"choices":[{"message":{"content":null,
            "tool_calls":[{"id":"call_a","function":{"arguments":"{}"}}]},
            "finish_reason":"tool_calls"}]}"#;

        let events = parse_completion(body).expect("valid body");
        assert_eq!(events.len(), 1);
        assert!(matches!(&events[0], ChatEvent::Done { stop_reason } if stop_reason == "end_turn"));
    }

    #[test]
    fn arguments_are_parsed_into_an_object_or_fall_back_to_empty() {
        assert_eq!(parse_arguments(r#"{"r":1.0}"#), json!({"r": 1.0}));
        assert_eq!(parse_arguments(r#""{\"r\":1.0}""#), json!({"r": 1.0}));
        assert_eq!(parse_arguments(""), json!({}));
        assert_eq!(parse_arguments("{broken"), json!({}));
        assert_eq!(parse_arguments("[1,2]"), json!({}));
        assert_eq!(parse_arguments("null"), json!({}));
    }

    #[test]
    fn finish_reasons_map_onto_claude_stop_reasons() {
        assert_eq!(stop_reason(Some("stop"), false), "end_turn");
        assert_eq!(stop_reason(Some("length"), false), "max_tokens");
        assert_eq!(stop_reason(Some("content_filter"), false), "refusal");
        assert_eq!(stop_reason(Some("tool_calls"), true), "tool_use");
        assert_eq!(stop_reason(Some("stop"), true), "tool_use");
        assert_eq!(stop_reason(Some("tool_calls"), false), "end_turn");
        assert_eq!(stop_reason(Some("something_new"), false), "end_turn");
        assert_eq!(stop_reason(None, false), "end_turn");
    }

    #[test]
    fn error_in_a_successful_response_is_reported() {
        let body = r#"{"error":{"message":"Provider returned error","code":502}}"#;
        let err = parse_completion(body).expect_err("must fail");
        assert!(
            matches!(err, ChatError::Provider(message) if message == "Provider returned error")
        );
    }

    #[test]
    fn response_without_choices_or_valid_json_is_a_parse_error() {
        assert!(matches!(
            parse_completion(r#"{"choices":[]}"#).expect_err("must fail"),
            ChatError::ParseError(_)
        ));
        assert!(matches!(
            parse_completion("<html>").expect_err("must fail"),
            ChatError::ParseError(_)
        ));
    }

    // ── Actionable failure messages ─────────────────────────────────

    const OLLAMA: &str = "http://localhost:11434/v1";
    const HOSTED: &str = "https://openrouter.ai/api/v1";

    fn status(status: u16, body: &str) -> Failure {
        Failure::Status {
            status,
            body: body.to_string(),
        }
    }

    #[test]
    fn unreachable_ollama_says_how_to_start_it_and_pull_the_model() {
        let message =
            explain_failure(OLLAMA, "qwen2.5:7b", false, &Failure::Unreachable).expect("explained");
        assert!(message.contains(OLLAMA), "got: {message}");
        assert!(message.contains("ollama serve"), "got: {message}");
        assert!(message.contains("ollama pull qwen2.5:7b"), "got: {message}");
    }

    #[test]
    fn unreachable_hosted_server_points_at_the_base_url_setting() {
        let message = explain_failure(HOSTED, "vendor/model", true, &Failure::Unreachable)
            .expect("explained");
        assert!(message.contains(HOSTED), "got: {message}");
        assert!(message.contains("LLM_BASE_URL"), "got: {message}");
        assert!(!message.contains("ollama"), "got: {message}");
    }

    #[test]
    fn model_not_pulled_in_ollama_gives_the_pull_command() {
        let body = r#"{"error":{"message":"model \"qwen2.5:7b\" not found, try pulling it first","type":"api_error","param":null,"code":null}}"#;
        let message =
            explain_failure(OLLAMA, "qwen2.5:7b", false, &status(404, body)).expect("explained");
        assert_eq!(
            message,
            "The model `qwen2.5:7b` is not downloaded. Run `ollama pull qwen2.5:7b` and try again."
        );
    }

    #[test]
    fn model_missing_on_a_hosted_server_points_at_the_model_setting() {
        let body = r#"{"error":{"message":"No endpoints found for model vendor/model"}}"#;
        let message =
            explain_failure(HOSTED, "vendor/model", true, &status(404, body)).expect("explained");
        assert!(message.contains("vendor/model"), "got: {message}");
        assert!(message.contains("LLM_MODEL"), "got: {message}");
    }

    #[test]
    fn wrong_api_root_points_at_the_base_url_setting() {
        let message = explain_failure(
            "http://localhost:11434",
            "qwen2.5:7b",
            false,
            &status(404, "404 page not found"),
        )
        .expect("explained");
        assert!(message.contains("LLM_BASE_URL"), "got: {message}");
        assert!(message.contains("/v1"), "got: {message}");
    }

    #[test]
    fn model_without_tool_support_says_to_pick_another_model() {
        let body = r#"{"error":{"message":"registry.ollama.ai/library/gemma3:4b does not support tools","type":"api_error"}}"#;
        let message =
            explain_failure(OLLAMA, "gemma3:4b", false, &status(400, body)).expect("explained");
        assert!(message.contains("gemma3:4b"), "got: {message}");
        assert!(message.contains("LLM_MODEL"), "got: {message}");
    }

    #[test]
    fn rejected_or_missing_key_points_at_the_key_setting() {
        let rejected = explain_failure(HOSTED, "m", true, &status(401, "")).expect("explained");
        assert!(rejected.contains("rejected the API key"), "got: {rejected}");
        assert!(rejected.contains("LLM_API_KEY"), "got: {rejected}");

        let missing = explain_failure(HOSTED, "m", false, &status(403, "")).expect("explained");
        assert!(missing.contains("requires an API key"), "got: {missing}");
        assert!(missing.contains("LLM_API_KEY"), "got: {missing}");
    }

    #[test]
    fn other_failures_are_left_to_the_generic_error() {
        assert!(explain_failure(OLLAMA, "m", false, &status(500, "boom")).is_none());
        assert!(explain_failure(OLLAMA, "m", false, &status(400, "bad request")).is_none());
        assert!(explain_failure(HOSTED, "m", true, &status(429, "slow down")).is_none());
    }
}
