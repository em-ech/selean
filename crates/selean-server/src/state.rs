//! Application state shared across request handlers.

use std::sync::Arc;

use sqlx::PgPool;

use crate::auth::jwt::JwtConfig;
use crate::storage::StorageBackend;

/// Shared application state for the Axum server.
#[derive(Clone)]
pub struct AppState {
    /// Claude API key. Read from `ANTHROPIC_API_KEY` env var. When `None`,
    /// the server still runs and `/api/chat` reports that AI chat is disabled.
    pub api_key: Option<Arc<str>>,
    /// Claude model to use (e.g. `claude-sonnet-4-6`).
    pub model: Arc<str>,
    /// HTTP client for Claude API requests.
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
    /// Returns an error if the configured storage backend cannot be initialized.
    pub fn from_env() -> Result<Self, AppStateError> {
        let api_key = std::env::var("ANTHROPIC_API_KEY")
            .ok()
            .filter(|s| !s.is_empty())
            .map(Arc::from);

        let model =
            std::env::var("ANTHROPIC_MODEL").unwrap_or_else(|_| "claude-sonnet-4-6".to_string());

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
            api_key,
            model: Arc::from(model),
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

    /// Creates app state with explicit values (for testing).
    #[cfg(test)]
    pub fn new_test() -> Self {
        Self {
            api_key: Some(Arc::from("test-key")),
            model: Arc::from("claude-sonnet-4-6"),
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
}

/// Errors from app state initialization.
#[derive(Debug, thiserror::Error)]
pub enum AppStateError {
    /// Database pool was requested but `DATABASE_URL` was not configured.
    #[error("DATABASE_URL not configured; database features are unavailable")]
    NoDatabaseConfigured,
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
    fn test_state_defaults() {
        let state = AppState::new_test();
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
