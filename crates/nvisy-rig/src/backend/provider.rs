//! LLM provider connection parameters.
//!
//! [`Provider`] is a plain enum carrying API keys, model names, and optional
//! base URLs. The actual rig-core client is constructed lazily when a
//! [`BaseAgent`](super::BaseAgent) is built.

use std::time::Duration;

use reqwest_middleware::ClientBuilder;
use reqwest_middleware::ClientWithMiddleware;
use reqwest_retry::{RetryTransientMiddleware, policies::ExponentialBackoff};
use reqwest_tracing::TracingMiddleware;

/// Provider that requires an API key (OpenAI, Anthropic, Gemini).
#[derive(Clone)]
pub struct AuthenticatedProvider {
    pub api_key: String,
    pub model: String,
    pub base_url: Option<String>,
}

/// Provider that does not require an API key (Ollama).
#[derive(Clone)]
pub struct UnauthenticatedProvider {
    pub model: String,
    pub base_url: Option<String>,
}

/// Supported LLM providers.
///
/// Each variant holds connection parameters and the model name. The actual
/// rig client is constructed lazily when an agent is built.
///
/// # Example
/// ```rust,ignore
/// let provider = Provider::openai("sk-...", "gpt-4o");
/// let agent = NerAgent::new(&provider, config);
/// ```
#[derive(Clone)]
pub enum Provider {
    /// OpenAI (GPT-4o, GPT-4, etc.)
    OpenAi(AuthenticatedProvider),
    /// Anthropic (Claude)
    Anthropic(AuthenticatedProvider),
    /// Google Gemini
    Gemini(AuthenticatedProvider),
    /// Ollama (local models)
    Ollama(UnauthenticatedProvider),
}

impl Provider {
    /// Create an OpenAI provider.
    pub fn openai(api_key: &str, model: &str) -> Self {
        Self::OpenAi(AuthenticatedProvider {
            api_key: api_key.to_owned(),
            model: model.to_owned(),
            base_url: None,
        })
    }

    /// Create an Anthropic provider.
    pub fn anthropic(api_key: &str, model: &str) -> Self {
        Self::Anthropic(AuthenticatedProvider {
            api_key: api_key.to_owned(),
            model: model.to_owned(),
            base_url: None,
        })
    }

    /// Create a Google Gemini provider.
    pub fn gemini(api_key: &str, model: &str) -> Self {
        Self::Gemini(AuthenticatedProvider {
            api_key: api_key.to_owned(),
            model: model.to_owned(),
            base_url: None,
        })
    }

    /// Create an Ollama provider using the default local URL.
    pub fn ollama(model: &str) -> Self {
        Self::Ollama(UnauthenticatedProvider {
            model: model.to_owned(),
            base_url: None,
        })
    }

    /// Create an Ollama provider with a custom base URL.
    pub fn ollama_with_url(model: &str, url: &str) -> Self {
        Self::Ollama(UnauthenticatedProvider {
            model: model.to_owned(),
            base_url: Some(url.to_owned()),
        })
    }

    /// The model name for this provider.
    pub fn model(&self) -> &str {
        match self {
            Self::OpenAi(p) | Self::Anthropic(p) | Self::Gemini(p) => &p.model,
            Self::Ollama(p) => &p.model,
        }
    }
}

/// Build a `ClientWithMiddleware` with timeout, retry, and tracing middleware.
pub(crate) fn build_http_client(max_retries: u32) -> ClientWithMiddleware {
    let retry_policy = ExponentialBackoff::builder()
        .build_with_max_retries(max_retries);

    let client = reqwest_middleware::reqwest::Client::builder()
        .timeout(Duration::from_secs(120))
        .build()
        .expect("failed to build reqwest client");

    ClientBuilder::new(client)
        .with(TracingMiddleware::default())
        .with(RetryTransientMiddleware::new_with_policy(retry_policy))
        .build()
}
