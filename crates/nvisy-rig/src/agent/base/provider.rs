//! LLM provider connection parameters.
//!
//! [`Provider`] is a plain data enum carrying API keys and optional base
//! URLs. Client construction is deferred until an agent or backend is built.

use rig::client::Nothing;
use rig::providers::{anthropic, gemini, ollama, openai};

use crate::error::Error;

/// Provider that requires an API key (OpenAI, Anthropic, Gemini).
#[derive(Clone)]
pub struct AuthenticatedProvider {
    pub api_key: String,
    pub base_url: Option<String>,
}

/// Provider that does not require an API key (Ollama).
#[derive(Clone)]
pub struct UnauthenticatedProvider {
    pub base_url: Option<String>,
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
        })
    }

    /// Create an Anthropic provider from an API key.
    pub fn anthropic(api_key: &str) -> Self {
        Self::Anthropic(AuthenticatedProvider {
            api_key: api_key.to_owned(),
            base_url: None,
        })
    }

    /// Create a Google Gemini provider from an API key.
    pub fn gemini(api_key: &str) -> Self {
        Self::Gemini(AuthenticatedProvider {
            api_key: api_key.to_owned(),
            base_url: None,
        })
    }

    /// Create an Ollama provider using the default local URL.
    pub fn ollama() -> Self {
        Self::Ollama(UnauthenticatedProvider { base_url: None })
    }

    /// Create an Ollama provider with a custom base URL.
    pub fn ollama_with_url(url: &str) -> Self {
        Self::Ollama(UnauthenticatedProvider {
            base_url: Some(url.to_owned()),
        })
    }
}

/// Internal helper — builds a concrete rig client from connection params.
pub(crate) enum ProviderClient {
    OpenAi(openai::CompletionsClient),
    Anthropic(anthropic::Client),
    Gemini(gemini::Client),
    Ollama(ollama::Client),
}

impl ProviderClient {
    pub(crate) fn from_provider(provider: &Provider) -> Result<Self, Error> {
        match provider {
            Provider::OpenAi(p) => {
                let mut builder = openai::Client::builder().api_key(&p.api_key);
                if let Some(url) = &p.base_url {
                    builder = builder.base_url(url);
                }
                let client = builder
                    .build()
                    .map_err(|e| Error::Client(e.to_string()))?;
                Ok(Self::OpenAi(client.completions_api()))
            }
            Provider::Anthropic(p) => {
                let mut builder = anthropic::Client::builder().api_key(&p.api_key);
                if let Some(url) = &p.base_url {
                    builder = builder.base_url(url);
                }
                Ok(Self::Anthropic(
                    builder.build().map_err(|e| Error::Client(e.to_string()))?,
                ))
            }
            Provider::Gemini(p) => {
                let mut builder = gemini::Client::builder().api_key(&p.api_key);
                if let Some(url) = &p.base_url {
                    builder = builder.base_url(url);
                }
                Ok(Self::Gemini(
                    builder.build().map_err(|e| Error::Client(e.to_string()))?,
                ))
            }
            Provider::Ollama(p) => {
                let mut builder = ollama::Client::builder().api_key(Nothing);
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
