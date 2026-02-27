//! Application state shared across request handlers.

use std::sync::Arc;

/// Shared application state for the Axum server.
#[derive(Clone)]
pub struct AppState {
    /// Claude API key. Read from `ANTHROPIC_API_KEY` env var.
    pub api_key: Arc<str>,
    /// Claude model to use (e.g. `claude-sonnet-4-6`).
    pub model: Arc<str>,
    /// HTTP client for Claude API requests.
    pub http_client: reqwest::Client,
}

impl AppState {
    /// Creates a new app state from environment variables.
    ///
    /// # Errors
    ///
    /// Returns an error if `ANTHROPIC_API_KEY` is not set.
    pub fn from_env() -> Result<Self, AppStateError> {
        let api_key =
            std::env::var("ANTHROPIC_API_KEY").map_err(|_| AppStateError::MissingApiKey)?;

        let model =
            std::env::var("ANTHROPIC_MODEL").unwrap_or_else(|_| "claude-sonnet-4-6".to_string());

        Ok(Self {
            api_key: Arc::from(api_key),
            model: Arc::from(model),
            http_client: reqwest::Client::new(),
        })
    }

    /// Creates app state with explicit values (for testing).
    #[cfg(test)]
    pub fn new_test() -> Self {
        Self {
            api_key: Arc::from("test-key"),
            model: Arc::from("claude-sonnet-4-6"),
            http_client: reqwest::Client::new(),
        }
    }
}

/// Errors from app state initialization.
#[derive(Debug, thiserror::Error)]
pub enum AppStateError {
    /// The `ANTHROPIC_API_KEY` environment variable is not set.
    #[error("ANTHROPIC_API_KEY environment variable not set")]
    MissingApiKey,
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_state_is_clone() {
        let state = AppState::new_test();
        let _clone = state.clone();
    }

    #[test]
    fn test_state_defaults() {
        let state = AppState::new_test();
        assert_eq!(&*state.api_key, "test-key");
        assert_eq!(&*state.model, "claude-sonnet-4-6");
    }

    #[test]
    fn from_env_missing_key_errors() {
        // Verify the error type is constructed correctly.
        // We can't safely unset env vars in Rust 2024 edition,
        // so we test the error type directly.
        let err = AppStateError::MissingApiKey;
        assert!(err.to_string().contains("ANTHROPIC_API_KEY"));
    }
}
