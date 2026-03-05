//! Provider configuration for LLM agents.

use serde::{Deserialize, Serialize};

#[cfg(any(feature = "openai", feature = "anthropic", feature = "gemini"))]
use crate::backend::AuthenticatedProvider;
use crate::backend::UnauthenticatedProvider;

/// Supported LLM providers for agent-based tasks (NER, CV, OCR, text generation).
///
/// Each variant holds connection parameters and the model name. The actual
/// rig client is constructed lazily when an agent is built.
///
/// # Example
/// ```rust,ignore
/// let provider = AgentProvider::openai("sk-...", "gpt-4o");
/// let agent = NerAgent::new(&provider, config);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AgentProvider {
    /// OpenAI (GPT-4o, GPT-4, etc.)
    #[cfg(feature = "openai")]
    #[cfg_attr(docsrs, doc(cfg(feature = "openai")))]
    OpenAi(AuthenticatedProvider),
    /// Anthropic (Claude)
    #[cfg(feature = "anthropic")]
    #[cfg_attr(docsrs, doc(cfg(feature = "anthropic")))]
    Anthropic(AuthenticatedProvider),
    /// Google Gemini
    #[cfg(feature = "gemini")]
    #[cfg_attr(docsrs, doc(cfg(feature = "gemini")))]
    Gemini(AuthenticatedProvider),
    /// Ollama (local models)
    Ollama(UnauthenticatedProvider),
}

impl AgentProvider {
    /// Create an OpenAI provider.
    #[cfg(feature = "openai")]
    #[cfg_attr(docsrs, doc(cfg(feature = "openai")))]
    pub fn openai(api_key: &str, model: &str) -> Self {
        Self::OpenAi(AuthenticatedProvider {
            api_key: api_key.to_owned(),
            model: model.to_owned(),
            base_url: None,
        })
    }

    /// Create an Anthropic provider.
    #[cfg(feature = "anthropic")]
    #[cfg_attr(docsrs, doc(cfg(feature = "anthropic")))]
    pub fn anthropic(api_key: &str, model: &str) -> Self {
        Self::Anthropic(AuthenticatedProvider {
            api_key: api_key.to_owned(),
            model: model.to_owned(),
            base_url: None,
        })
    }

    /// Create a Google Gemini provider.
    #[cfg(feature = "gemini")]
    #[cfg_attr(docsrs, doc(cfg(feature = "gemini")))]
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
            #[cfg(feature = "openai")]
            Self::OpenAi(p) => &p.model,
            #[cfg(feature = "anthropic")]
            Self::Anthropic(p) => &p.model,
            #[cfg(feature = "gemini")]
            Self::Gemini(p) => &p.model,
            Self::Ollama(p) => &p.model,
        }
    }
}
