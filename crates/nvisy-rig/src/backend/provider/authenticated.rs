//! LLM providers that require an API key.

use std::fmt;

#[cfg(any(
    feature = "openai-gpt",
    feature = "openai-whisper",
    feature = "openai-tts",
    feature = "anthropic-claude",
    feature = "google-gemini"
))]
use reqwest_middleware::ClientWithMiddleware;
#[cfg(feature = "anthropic-claude")]
use rig::providers::anthropic;
#[cfg(feature = "google-gemini")]
use rig::providers::gemini;
#[cfg(any(
    feature = "openai-gpt",
    feature = "openai-whisper",
    feature = "openai-tts"
))]
use rig::providers::openai;
use serde::{Deserialize, Serialize};

#[cfg(any(
    feature = "openai-gpt",
    feature = "openai-whisper",
    feature = "openai-tts",
    feature = "anthropic-claude",
    feature = "google-gemini"
))]
use crate::error::Error;

/// Provider that requires an API key (OpenAI, Anthropic, Gemini).
#[derive(Clone, Serialize, Deserialize)]
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
    #[cfg(any(
        feature = "openai-gpt",
        feature = "openai-whisper",
        feature = "openai-tts"
    ))]
    #[cfg_attr(
        docsrs,
        doc(cfg(any(
            feature = "openai-gpt",
            feature = "openai-whisper",
            feature = "openai-tts"
        )))
    )]
    pub(crate) fn openai_client(
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
    pub(crate) fn gemini_client(
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
    pub(crate) fn anthropic_client(
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
