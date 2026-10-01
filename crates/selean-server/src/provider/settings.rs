//! Chat provider settings read from the environment.

use std::str::FromStr;
use std::sync::Arc;

use super::Provider;
use super::{anthropic, openai_compatible};

/// Selects the provider: `openai-compatible` (default) or `anthropic`.
pub const PROVIDER_VAR: &str = "LLM_PROVIDER";
/// API root of the OpenAI-compatible server.
pub const BASE_URL_VAR: &str = "LLM_BASE_URL";
/// API key for the OpenAI-compatible server. Optional.
pub const API_KEY_VAR: &str = "LLM_API_KEY";
/// Model served by the OpenAI-compatible server.
pub const MODEL_VAR: &str = "LLM_MODEL";
/// API key for the Claude API.
pub const ANTHROPIC_API_KEY_VAR: &str = "ANTHROPIC_API_KEY";
/// Claude model.
pub const ANTHROPIC_MODEL_VAR: &str = "ANTHROPIC_MODEL";

/// The provider the AI chat uses and how to reach it.
#[derive(Clone)]
pub struct LlmSettings {
    /// Selected provider.
    pub provider: Provider,
    /// API root of the OpenAI-compatible server, without a trailing slash.
    /// `None` for Anthropic, whose endpoint is fixed.
    pub base_url: Option<Arc<str>>,
    /// API key for the selected provider, if one is set.
    pub api_key: Option<Arc<str>>,
    /// Model name sent to the provider.
    pub model: Arc<str>,
}

impl LlmSettings {
    /// Reads the settings from the process environment.
    ///
    /// # Errors
    ///
    /// Returns an error if `LLM_PROVIDER` names an unknown provider.
    pub fn from_env() -> Result<Self, UnknownProviderError> {
        Self::resolve(|name| std::env::var(name).ok())
    }

    /// Resolves the settings from a variable lookup. Blank values count as
    /// unset.
    ///
    /// With nothing set, the result is the keyless local default: an
    /// OpenAI-compatible Ollama server on this machine. Each provider reads
    /// only its own key, so a key is never sent to a host it was not issued
    /// for, and a paid provider is used only when it is selected by name.
    ///
    /// # Errors
    ///
    /// Returns an error if `LLM_PROVIDER` names an unknown provider.
    pub fn resolve(lookup: impl Fn(&str) -> Option<String>) -> Result<Self, UnknownProviderError> {
        let get = |name: &str| {
            lookup(name)
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
        };

        let provider = match get(PROVIDER_VAR) {
            Some(name) => name.parse()?,
            None => Provider::default(),
        };

        Ok(match provider {
            Provider::OpenAiCompatible => {
                let base_url =
                    get(BASE_URL_VAR).unwrap_or_else(|| openai_compatible::DEFAULT_BASE_URL.into());
                Self {
                    provider,
                    base_url: Some(Arc::from(base_url.trim_end_matches('/'))),
                    api_key: get(API_KEY_VAR).map(Arc::from),
                    model: Arc::from(
                        get(MODEL_VAR).unwrap_or_else(|| openai_compatible::DEFAULT_MODEL.into()),
                    ),
                }
            }
            Provider::Anthropic => Self {
                provider,
                base_url: None,
                api_key: get(ANTHROPIC_API_KEY_VAR).map(Arc::from),
                model: Arc::from(
                    get(ANTHROPIC_MODEL_VAR).unwrap_or_else(|| anthropic::DEFAULT_MODEL.into()),
                ),
            },
        })
    }
}

/// Hand-written so that the API key never reaches a log or a test failure.
impl std::fmt::Debug for LlmSettings {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LlmSettings")
            .field("provider", &self.provider)
            .field("base_url", &self.base_url)
            .field("api_key", &self.api_key.as_ref().map(|_| "<redacted>"))
            .field("model", &self.model)
            .finish()
    }
}

impl FromStr for Provider {
    type Err = UnknownProviderError;

    fn from_str(name: &str) -> Result<Self, Self::Err> {
        match name.to_ascii_lowercase().as_str() {
            "openai-compatible" => Ok(Self::OpenAiCompatible),
            "anthropic" => Ok(Self::Anthropic),
            _ => Err(UnknownProviderError(name.to_string())),
        }
    }
}

/// `LLM_PROVIDER` names a provider that does not exist.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
#[error("unknown LLM_PROVIDER `{0}`; use `openai-compatible` or `anthropic`")]
pub struct UnknownProviderError(String);

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn resolve(vars: &[(&str, &str)]) -> Result<LlmSettings, UnknownProviderError> {
        LlmSettings::resolve(|name| {
            vars.iter()
                .find(|(key, _)| *key == name)
                .map(|(_, value)| (*value).to_string())
        })
    }

    #[test]
    fn nothing_set_selects_keyless_local_ollama() {
        let settings = resolve(&[]).expect("defaults are valid");
        assert_eq!(settings.provider, Provider::OpenAiCompatible);
        assert_eq!(
            settings.base_url.as_deref(),
            Some("http://localhost:11434/v1")
        );
        assert!(settings.api_key.is_none());
        assert_eq!(&*settings.model, "qwen2.5:7b");
    }

    #[test]
    fn anthropic_key_alone_does_not_select_anthropic_or_leak_to_the_local_server() {
        let settings = resolve(&[(ANTHROPIC_API_KEY_VAR, "anthropic-key")]).expect("valid");
        assert_eq!(settings.provider, Provider::OpenAiCompatible);
        assert!(settings.api_key.is_none());
    }

    #[test]
    fn anthropic_is_selected_by_name_and_reads_its_own_variables() {
        let settings = resolve(&[
            (PROVIDER_VAR, "anthropic"),
            (ANTHROPIC_API_KEY_VAR, "anthropic-key"),
            (API_KEY_VAR, "other-key"),
            (MODEL_VAR, "some/open-model"),
        ])
        .expect("valid");
        assert_eq!(settings.provider, Provider::Anthropic);
        assert!(settings.base_url.is_none());
        assert_eq!(settings.api_key.as_deref(), Some("anthropic-key"));
        assert_eq!(&*settings.model, "claude-sonnet-4-6");
    }

    #[test]
    fn anthropic_model_can_be_overridden() {
        let settings = resolve(&[
            (PROVIDER_VAR, "Anthropic"),
            (ANTHROPIC_MODEL_VAR, "claude-other"),
        ])
        .expect("valid");
        assert_eq!(&*settings.model, "claude-other");
        assert!(settings.api_key.is_none());
    }

    #[test]
    fn openai_compatible_host_reads_base_url_key_and_model() {
        let settings = resolve(&[
            (PROVIDER_VAR, "openai-compatible"),
            (BASE_URL_VAR, "https://openrouter.ai/api/v1/"),
            (API_KEY_VAR, "router-key"),
            (MODEL_VAR, "vendor/model"),
            (ANTHROPIC_API_KEY_VAR, "anthropic-key"),
        ])
        .expect("valid");
        assert_eq!(settings.provider, Provider::OpenAiCompatible);
        assert_eq!(
            settings.base_url.as_deref(),
            Some("https://openrouter.ai/api/v1")
        );
        assert_eq!(settings.api_key.as_deref(), Some("router-key"));
        assert_eq!(&*settings.model, "vendor/model");
    }

    #[test]
    fn blank_values_count_as_unset() {
        let settings = resolve(&[
            (PROVIDER_VAR, "  "),
            (BASE_URL_VAR, ""),
            (API_KEY_VAR, " "),
            (MODEL_VAR, ""),
        ])
        .expect("valid");
        assert_eq!(settings.provider, Provider::OpenAiCompatible);
        assert_eq!(
            settings.base_url.as_deref(),
            Some("http://localhost:11434/v1")
        );
        assert!(settings.api_key.is_none());
        assert_eq!(&*settings.model, "qwen2.5:7b");
    }

    #[test]
    fn debug_output_redacts_the_api_key() {
        let settings = resolve(&[(API_KEY_VAR, "router-key")]).expect("valid");
        let debug = format!("{settings:?}");
        assert!(!debug.contains("router-key"), "got: {debug}");
        assert!(debug.contains("<redacted>"), "got: {debug}");
    }

    #[test]
    fn unknown_provider_is_an_error_that_names_the_valid_values() {
        let err = resolve(&[(PROVIDER_VAR, "openrouter")]).expect_err("unknown provider must fail");
        let message = err.to_string();
        assert!(message.contains("openrouter"), "got: {message}");
        assert!(message.contains("openai-compatible"), "got: {message}");
        assert!(message.contains("anthropic"), "got: {message}");
    }
}
