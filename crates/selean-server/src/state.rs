//! Application state shared across request handlers.

use std::sync::Arc;

use sqlx::PgPool;

use crate::auth::jwt::JwtConfig;
use crate::provider::Provider;
use crate::provider::settings::{LlmSettings, UnknownProviderError};
#[cfg(test)]
use crate::provider::{anthropic, openai_compatible};
use crate::storage::StorageBackend;

/// Shared application state for the Axum server.
#[derive(Clone)]
pub struct AppState {
    /// LLM provider that serves the AI chat. Read from `LLM_PROVIDER`;
    /// defaults to an OpenAI-compatible local Ollama server, which needs no key.
    pub llm_provider: Provider,
    /// API root of the OpenAI-compatible server. Read from `LLM_BASE_URL`.
    /// `None` for Anthropic.
    pub llm_base_url: Option<Arc<str>>,
    /// API key for the selected provider: `LLM_API_KEY` (optional) for an
    /// OpenAI-compatible server, `ANTHROPIC_API_KEY` for Anthropic.
    pub api_key: Option<Arc<str>>,
    /// Model to use: `LLM_MODEL` for an OpenAI-compatible server,
    /// `ANTHROPIC_MODEL` for Anthropic.
    pub model: Arc<str>,
    /// HTTP client for LLM provider requests.
    pub http_client: reqwest::Client,
    /// Figma personal access token. Read from `FIGMA_ACCESS_TOKEN` env var.
    pub figma_access_token: Option<Arc<str>>,
    /// `InDesign` Server URL for `.indd` to IDML conversion. Read from
    /// `INDESIGN_SERVER_URL` env var. When `None`, `.indd` import returns 501.
    pub indesign_server_url: Option<Arc<str>>,
    /// `PostgreSQL` connection pool. `None` when `DATABASE_URL` is not set
    /// (local dev without database).
    pub db: Option<PgPool>,
    /// JWT configuration for token-based auth. `None` when `JWT_SECRET` is not set.
    pub jwt: Option<JwtConfig>,
    /// Asset storage backend (local filesystem or S3).
    pub storage: Arc<dyn StorageBackend>,
    /// GitHub OAuth client ID. Read from `GITHUB_CLIENT_ID` env var.
    pub github_client_id: Option<Arc<str>>,
    /// GitHub OAuth client secret. Read from `GITHUB_CLIENT_SECRET` env var.
    pub github_client_secret: Option<Arc<str>>,
}

impl AppState {
    /// Creates a new app state from environment variables.
    ///
    /// # Errors
    ///
    /// Returns an error if `LLM_PROVIDER` names an unknown provider or the
    /// configured storage backend cannot be initialized.
    pub fn from_env() -> Result<Self, AppStateError> {
        let llm = LlmSettings::from_env()?;

        let figma_access_token = std::env::var("FIGMA_ACCESS_TOKEN")
            .ok()
            .filter(|s| !s.is_empty())
            .map(Arc::from);

        let indesign_server_url = std::env::var("INDESIGN_SERVER_URL")
            .ok()
            .filter(|s| !s.is_empty())
            .map(Arc::from);

        let storage: Arc<dyn StorageBackend> =
            if let Some(s3_config) = crate::storage::s3::S3Config::from_env() {
                Arc::new(
                    crate::storage::s3::S3Storage::new(&s3_config)
                        .map_err(|e| AppStateError::StorageInit(e.to_string()))?,
                )
            } else {
                let asset_dir =
                    std::env::var("SELEAN_ASSET_DIR").unwrap_or_else(|_| "data/assets".to_string());
                Arc::new(crate::storage::local::LocalStorage::new(asset_dir))
            };

        Ok(Self {
            llm_provider: llm.provider,
            llm_base_url: llm.base_url,
            api_key: llm.api_key,
            model: llm.model,
            http_client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(120))
                .build()
                .unwrap_or_default(),
            figma_access_token,
            indesign_server_url,
            db: None,
            jwt: JwtConfig::from_env(),
            storage,
            github_client_id: std::env::var("GITHUB_CLIENT_ID")
                .ok()
                .filter(|s| !s.is_empty())
                .map(Arc::from),
            github_client_secret: std::env::var("GITHUB_CLIENT_SECRET")
                .ok()
                .filter(|s| !s.is_empty())
                .map(Arc::from),
        })
    }

    /// Sets the database pool on this state.
    #[must_use]
    pub fn with_db(mut self, pool: PgPool) -> Self {
        self.db = Some(pool);
        self
    }

    /// Returns a reference to the database pool, or an error if not configured.
    ///
    /// # Errors
    ///
    /// Returns `AppStateError::NoDatabaseConfigured` if `DATABASE_URL` was not set.
    pub fn require_db(&self) -> Result<&PgPool, AppStateError> {
        self.db.as_ref().ok_or(AppStateError::NoDatabaseConfigured)
    }

    /// Creates app state for testing, with the default chat provider: a
    /// keyless OpenAI-compatible server at the local Ollama address.
    #[cfg(test)]
    pub fn new_test() -> Self {
        Self {
            llm_provider: Provider::OpenAiCompatible,
            llm_base_url: Some(Arc::from(openai_compatible::DEFAULT_BASE_URL)),
            api_key: None,
            model: Arc::from(openai_compatible::DEFAULT_MODEL),
            http_client: reqwest::Client::new(),
            figma_access_token: None,
            indesign_server_url: None,
            db: None,
            jwt: None,
            storage: Arc::new(crate::storage::local::LocalStorage::new(
                "/tmp/selean-test-assets",
            )),
            github_client_id: None,
            github_client_secret: None,
        }
    }

    /// Creates app state for testing with Anthropic selected and a key set.
    #[cfg(test)]
    pub fn new_test_anthropic() -> Self {
        Self {
            llm_provider: Provider::Anthropic,
            llm_base_url: None,
            api_key: Some(Arc::from("test-key")),
            model: Arc::from(anthropic::DEFAULT_MODEL),
            ..Self::new_test()
        }
    }
}

/// Errors from app state initialization.
#[derive(Debug, thiserror::Error)]
pub enum AppStateError {
    /// Database pool was requested but `DATABASE_URL` was not configured.
    #[error("DATABASE_URL not configured; database features are unavailable")]
    NoDatabaseConfigured,
    /// `LLM_PROVIDER` names an unknown provider.
    #[error(transparent)]
    UnknownProvider(#[from] UnknownProviderError),
    /// Storage backend initialization failed.
    #[error("storage initialization failed: {0}")]
    StorageInit(String),
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
    fn test_state_defaults_to_keyless_local_provider() {
        let state = AppState::new_test();
        assert_eq!(state.llm_provider, Provider::OpenAiCompatible);
        assert_eq!(
            state.llm_base_url.as_deref(),
            Some("http://localhost:11434/v1")
        );
        assert!(state.api_key.is_none());
        assert_eq!(&*state.model, "qwen2.5:7b");
    }

    #[test]
    fn test_state_anthropic_has_key_and_claude_model() {
        let state = AppState::new_test_anthropic();
        assert_eq!(state.llm_provider, Provider::Anthropic);
        assert!(state.llm_base_url.is_none());
        assert_eq!(state.api_key.as_deref(), Some("test-key"));
        assert_eq!(&*state.model, "claude-sonnet-4-6");
    }

    #[test]
    fn test_state_figma_token_is_none() {
        let state = AppState::new_test();
        assert!(state.figma_access_token.is_none());
    }

    #[test]
    fn test_state_indesign_server_url_is_none() {
        let state = AppState::new_test();
        assert!(state.indesign_server_url.is_none());
    }

    #[test]
    fn test_state_github_fields_are_none() {
        let state = AppState::new_test();
        assert!(state.github_client_id.is_none());
        assert!(state.github_client_secret.is_none());
    }

    #[test]
    fn test_state_db_is_none_by_default() {
        let state = AppState::new_test();
        assert!(state.db.is_none());
        assert!(state.require_db().is_err());
    }

    #[test]
    fn no_database_error_message() {
        let err = AppStateError::NoDatabaseConfigured;
        assert!(err.to_string().contains("DATABASE_URL"));
    }
}
