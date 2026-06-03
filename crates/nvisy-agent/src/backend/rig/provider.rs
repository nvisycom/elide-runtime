//! LLM provider selection: auth'd cloud providers + unauth'd local
//! providers, behind a single [`LlmProvider`] enum.

use std::fmt;

use reqwest_middleware::ClientWithMiddleware;
#[cfg(feature = "anthropic-claude")]
use rig::providers::anthropic;
#[cfg(feature = "google-gemini")]
use rig::providers::gemini;
use rig::providers::ollama;
#[cfg(feature = "openai-gpt")]
use rig::providers::openai;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::error::Error;

/// Supported LLM providers for agent-based tasks.
///
/// Each variant holds connection parameters and the model name. The
/// actual rig client is constructed lazily when a [`RigBackend`] is
/// built.
///
/// [`RigBackend`]: super::RigBackend
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum LlmProvider {
    /// OpenAI (GPT-4o, GPT-4, etc.).
    #[cfg(feature = "openai-gpt")]
    #[cfg_attr(docsrs, doc(cfg(feature = "openai-gpt")))]
    OpenAi(AuthenticatedProvider),
    /// Anthropic (Claude).
    #[cfg(feature = "anthropic-claude")]
    #[cfg_attr(docsrs, doc(cfg(feature = "anthropic-claude")))]
    Anthropic(AuthenticatedProvider),
    /// Google Gemini.
    #[cfg(feature = "google-gemini")]
    #[cfg_attr(docsrs, doc(cfg(feature = "google-gemini")))]
    Gemini(AuthenticatedProvider),
    /// Ollama (local models).
    Ollama(UnauthenticatedProvider),
}

impl LlmProvider {
    /// Create an OpenAI provider.
    #[cfg(feature = "openai-gpt")]
    #[cfg_attr(docsrs, doc(cfg(feature = "openai-gpt")))]
    pub fn openai(api_key: &str, model: &str) -> Self {
        Self::OpenAi(AuthenticatedProvider {
            api_key: api_key.to_owned(),
            model: model.to_owned(),
            base_url: None,
        })
    }

    /// Create an Anthropic provider.
    #[cfg(feature = "anthropic-claude")]
    #[cfg_attr(docsrs, doc(cfg(feature = "anthropic-claude")))]
    pub fn anthropic(api_key: &str, model: &str) -> Self {
        Self::Anthropic(AuthenticatedProvider {
            api_key: api_key.to_owned(),
            model: model.to_owned(),
            base_url: None,
        })
    }

    /// Create a Google Gemini provider.
    #[cfg(feature = "google-gemini")]
    #[cfg_attr(docsrs, doc(cfg(feature = "google-gemini")))]
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
            #[cfg(feature = "openai-gpt")]
            Self::OpenAi(p) => &p.model,
            #[cfg(feature = "anthropic-claude")]
            Self::Anthropic(p) => &p.model,
            #[cfg(feature = "google-gemini")]
            Self::Gemini(p) => &p.model,
            Self::Ollama(p) => &p.model,
        }
    }
}

/// Provider that requires an API key (OpenAI, Anthropic, Gemini).
#[derive(Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct AuthenticatedProvider {
    pub api_key: String,
    pub model: String,
    pub base_url: Option<String>,
}

impl fmt::Debug for AuthenticatedProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AuthenticatedProvider")
            .field("api_key", &"***")
            .field("model", &self.model)
            .field("base_url", &self.base_url)
            .finish()
    }
}

impl AuthenticatedProvider {
    /// Build an OpenAI rig-core client.
    #[cfg(feature = "openai-gpt")]
    #[cfg_attr(docsrs, doc(cfg(feature = "openai-gpt")))]
    pub(super) fn openai_client(
        &self,
        http: ClientWithMiddleware,
    ) -> Result<openai::Client<ClientWithMiddleware>, Error> {
        let mut b = openai::Client::builder()
            .api_key(&self.api_key)
            .http_client(http);
        if let Some(url) = &self.base_url {
            b = b.base_url(url);
        }
        b.build().map_err(|e| Error::Request(e.to_string()))
    }

    /// Build a Gemini rig-core client.
    #[cfg(feature = "google-gemini")]
    #[cfg_attr(docsrs, doc(cfg(feature = "google-gemini")))]
    pub(super) fn gemini_client(
        &self,
        http: ClientWithMiddleware,
    ) -> Result<gemini::Client<ClientWithMiddleware>, Error> {
        let mut b = gemini::Client::builder()
            .api_key(&self.api_key)
            .http_client(http);
        if let Some(url) = &self.base_url {
            b = b.base_url(url);
        }
        b.build().map_err(|e| Error::Request(e.to_string()))
    }

    /// Build an Anthropic rig-core client.
    #[cfg(feature = "anthropic-claude")]
    #[cfg_attr(docsrs, doc(cfg(feature = "anthropic-claude")))]
    pub(super) fn anthropic_client(
        &self,
        http: ClientWithMiddleware,
    ) -> Result<anthropic::Client<ClientWithMiddleware>, Error> {
        let mut b = anthropic::Client::builder()
            .api_key(&self.api_key)
            .http_client(http);
        if let Some(url) = &self.base_url {
            b = b.base_url(url);
        }
        b.build().map_err(|e| Error::Request(e.to_string()))
    }
}

/// Provider that does not require an API key (Ollama).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct UnauthenticatedProvider {
    pub model: String,
    pub base_url: Option<String>,
}

impl UnauthenticatedProvider {
    /// Build an Ollama rig-core client.
    pub(super) fn ollama_client(
        &self,
        http: ClientWithMiddleware,
    ) -> Result<ollama::Client<ClientWithMiddleware>, Error> {
        let mut b = ollama::Client::builder()
            .api_key(rig::client::Nothing)
            .http_client(http);
        if let Some(url) = &self.base_url {
            b = b.base_url(url);
        }
        b.build().map_err(|e| Error::Request(e.to_string()))
    }
}
