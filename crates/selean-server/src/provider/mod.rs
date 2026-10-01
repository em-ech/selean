//! LLM providers for the AI chat.
//!
//! Each provider translates the chat request into its own wire format and
//! translates the response back into the [`ChatEvent`] values the web app
//! consumes. [`crate::chat`] owns the HTTP round trip and is the only caller.

pub mod anthropic;
pub mod anthropic_stream;
pub mod openai_compatible;
pub mod openai_stream;
pub mod settings;
pub mod sse;

use crate::chat::{ChatError, ChatEvent, ChatMessage};
use crate::state::AppState;
use sse::EventParser;

/// Upper bound on the tokens a model may generate in one chat turn.
pub(crate) const MAX_OUTPUT_TOKENS: u32 = 4096;

/// What a provider must supply for [`crate::chat`] to run a chat turn.
pub trait ChatProvider: Sync {
    /// Builds the HTTP request for one chat turn: endpoint, auth headers and
    /// the translated body.
    ///
    /// # Errors
    ///
    /// Returns an error if the provider is not configured well enough to be
    /// called, so that no request is made.
    fn request(
        &self,
        state: &AppState,
        system: &str,
        messages: &[ChatMessage],
        stream: bool,
    ) -> Result<reqwest::RequestBuilder, ChatError>;

    /// Translates a complete (non-streaming) response body into chat events,
    /// ending with [`ChatEvent::Done`].
    ///
    /// # Errors
    ///
    /// Returns an error if the body is not a response this provider
    /// understands.
    fn parse_response(&self, body: &str) -> Result<Vec<ChatEvent>, ChatError>;

    /// Creates a parser for this provider's streaming response format.
    fn stream_parser(&self) -> Box<dyn EventParser>;

    /// Turns a failed request into a message that tells the user how to fix
    /// it, when the provider recognises the failure. `None` leaves the
    /// generic error in place.
    fn explain_failure(&self, state: &AppState, failure: &Failure) -> Option<String>;
}

/// A failed provider request, as passed to [`ChatProvider::explain_failure`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Failure {
    /// No connection could be made to the provider.
    Unreachable,
    /// The provider answered with an error status.
    Status {
        /// HTTP status code.
        status: u16,
        /// Response body.
        body: String,
    },
}

/// Which LLM provider serves the AI chat.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Provider {
    /// Any server that implements the chat completions API, by default a
    /// local Ollama server.
    #[default]
    OpenAiCompatible,
    /// The Claude API.
    Anthropic,
}

impl Provider {
    /// Returns the implementation of this provider.
    pub fn client(self) -> &'static dyn ChatProvider {
        match self {
            Self::OpenAiCompatible => &openai_compatible::OpenAiCompatible,
            Self::Anthropic => &anthropic::Anthropic,
        }
    }
}
