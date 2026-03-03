//! LLM providers that do not require an API key.

use reqwest_middleware::ClientWithMiddleware;
use rig::providers::ollama;

use crate::error::Error;

/// Provider that does not require an API key (Ollama).
#[derive(Debug, Clone)]
pub struct UnauthenticatedProvider {
    pub model: String,
    pub base_url: Option<String>,
}

impl UnauthenticatedProvider {
    /// Build an Ollama rig-core client.
    pub(crate) fn ollama_client(
        &self,
        http: ClientWithMiddleware,
    ) -> Result<ollama::Client<ClientWithMiddleware>, Error> {
        let mut b = ollama::Client::<ClientWithMiddleware>::builder()
            .api_key(rig::client::Nothing)
            .http_client(http);
        if let Some(url) = &self.base_url {
            b = b.base_url(url);
        }
        b.build().map_err(|e| Error::Client(e.to_string()))
    }
}
