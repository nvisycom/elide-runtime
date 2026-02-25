//! LLM provider connection parameters.
//!
//! [`Provider`] is a plain data enum carrying API keys and optional base
//! URLs. Client construction is deferred until an agent or backend is built.

use reqwest_middleware::ClientBuilder;
use reqwest_retry::{RetryTransientMiddleware, policies::ExponentialBackoff};
use rig::client::Nothing;
use rig::providers::{anthropic, gemini, ollama, openai};

use crate::error::Error;

/// HTTP client type used by all rig provider clients.
pub(crate) type HttpClient = reqwest_middleware::ClientWithMiddleware;

/// Default number of retries for transient HTTP errors.
const DEFAULT_MAX_RETRIES: u32 = 3;

/// Provider that requires an API key (OpenAI, Anthropic, Gemini).
#[derive(Clone)]
pub struct AuthenticatedProvider {
    pub api_key: String,
    pub base_url: Option<String>,
    /// Maximum retries for transient HTTP errors.
    pub max_retries: u32,
}

/// Provider that does not require an API key (Ollama).
#[derive(Clone)]
pub struct UnauthenticatedProvider {
    pub base_url: Option<String>,
    /// Maximum retries for transient HTTP errors.
    pub max_retries: u32,
}

/// Supported LLM providers.
///
/// Each variant holds only connection parameters. The actual rig client
/// is constructed lazily when an agent or backend is built.
///
/// # Example
/// ```rust,ignore
/// let provider = Provider::openai("sk-...");
/// let agent = NerAgent::new(&provider, "gpt-4o", config);
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
    /// Create an OpenAI provider from an API key.
    pub fn openai(api_key: &str) -> Self {
        Self::OpenAi(AuthenticatedProvider {
            api_key: api_key.to_owned(),
            base_url: None,
            max_retries: DEFAULT_MAX_RETRIES,
        })
    }

    /// Create an Anthropic provider from an API key.
    pub fn anthropic(api_key: &str) -> Self {
        Self::Anthropic(AuthenticatedProvider {
            api_key: api_key.to_owned(),
            base_url: None,
            max_retries: DEFAULT_MAX_RETRIES,
        })
    }

    /// Create a Google Gemini provider from an API key.
    pub fn gemini(api_key: &str) -> Self {
        Self::Gemini(AuthenticatedProvider {
            api_key: api_key.to_owned(),
            base_url: None,
            max_retries: DEFAULT_MAX_RETRIES,
        })
    }

    /// Create an Ollama provider using the default local URL.
    pub fn ollama() -> Self {
        Self::Ollama(UnauthenticatedProvider {
            base_url: None,
            max_retries: DEFAULT_MAX_RETRIES,
        })
    }

    /// Create an Ollama provider with a custom base URL.
    pub fn ollama_with_url(url: &str) -> Self {
        Self::Ollama(UnauthenticatedProvider {
            base_url: Some(url.to_owned()),
            max_retries: DEFAULT_MAX_RETRIES,
        })
    }
}

/// Build a `ClientWithMiddleware` with retry middleware.
fn build_http_client(max_retries: u32) -> HttpClient {
    let retry_policy = ExponentialBackoff::builder()
        .build_with_max_retries(max_retries);
    ClientBuilder::new(reqwest_middleware::reqwest::Client::new())
        .with(RetryTransientMiddleware::new_with_policy(retry_policy))
        .build()
}

/// Internal helper — builds a concrete rig client from connection params.
pub(crate) enum ProviderClient {
    OpenAi(openai::CompletionsClient<HttpClient>),
    Anthropic(anthropic::Client<HttpClient>),
    Gemini(gemini::Client<HttpClient>),
    Ollama(ollama::Client<HttpClient>),
}

impl ProviderClient {
    pub(crate) fn from_provider(provider: &Provider) -> Result<Self, Error> {
        match provider {
            Provider::OpenAi(p) => {
                let http_client = build_http_client(p.max_retries);
                let mut builder = openai::Client::<HttpClient>::builder()
                    .api_key(&p.api_key)
                    .http_client(http_client);
                if let Some(url) = &p.base_url {
                    builder = builder.base_url(url);
                }
                let client = builder
                    .build()
                    .map_err(|e| Error::Client(e.to_string()))?;
                Ok(Self::OpenAi(client.completions_api()))
            }
            Provider::Anthropic(p) => {
                let http_client = build_http_client(p.max_retries);
                let mut builder = anthropic::Client::<HttpClient>::builder()
                    .api_key(&p.api_key)
                    .http_client(http_client);
                if let Some(url) = &p.base_url {
                    builder = builder.base_url(url);
                }
                Ok(Self::Anthropic(
                    builder.build().map_err(|e| Error::Client(e.to_string()))?,
                ))
            }
            Provider::Gemini(p) => {
                let http_client = build_http_client(p.max_retries);
                let mut builder = gemini::Client::<HttpClient>::builder()
                    .api_key(&p.api_key)
                    .http_client(http_client);
                if let Some(url) = &p.base_url {
                    builder = builder.base_url(url);
                }
                Ok(Self::Gemini(
                    builder.build().map_err(|e| Error::Client(e.to_string()))?,
                ))
            }
            Provider::Ollama(p) => {
                let http_client = build_http_client(p.max_retries);
                let mut builder = ollama::Client::<HttpClient>::builder()
                    .api_key(Nothing)
                    .http_client(http_client);
                if let Some(url) = &p.base_url {
                    builder = builder.base_url(url);
                }
                Ok(Self::Ollama(
                    builder.build().map_err(|e| Error::Client(e.to_string()))?,
                ))
            }
        }
    }
}
