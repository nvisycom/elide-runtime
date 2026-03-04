//! LLM providers that require an API key.

use std::fmt;

use reqwest_middleware::ClientWithMiddleware;
use rig::providers::{anthropic, gemini, openai};
use serde::{Deserialize, Serialize};

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
    pub(crate) fn openai_client(
        &self,
        http: ClientWithMiddleware,
    ) -> Result<openai::Client<ClientWithMiddleware>, Error> {
        let mut b = openai::Client::<ClientWithMiddleware>::builder()
            .api_key(&self.api_key)
            .http_client(http);
        if let Some(url) = &self.base_url {
            b = b.base_url(url);
        }
        b.build().map_err(|e| Error::Request(e.to_string()))
    }

    /// Build a Gemini rig-core client.
    pub(crate) fn gemini_client(
        &self,
        http: ClientWithMiddleware,
    ) -> Result<gemini::Client<ClientWithMiddleware>, Error> {
        let mut b = gemini::Client::<ClientWithMiddleware>::builder()
            .api_key(&self.api_key)
            .http_client(http);
        if let Some(url) = &self.base_url {
            b = b.base_url(url);
        }
        b.build().map_err(|e| Error::Request(e.to_string()))
    }

    /// Build an Anthropic rig-core client.
    pub(crate) fn anthropic_client(
        &self,
        http: ClientWithMiddleware,
    ) -> Result<anthropic::Client<ClientWithMiddleware>, Error> {
        let mut b = anthropic::Client::<ClientWithMiddleware>::builder()
            .api_key(&self.api_key)
            .http_client(http);
        if let Some(url) = &self.base_url {
            b = b.base_url(url);
        }
        b.build().map_err(|e| Error::Request(e.to_string()))
    }
}
