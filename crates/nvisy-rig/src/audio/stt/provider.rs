//! Provider dispatch for speech-to-text models.

#[cfg(feature = "openai-whisper")]
use nvisy_http::{HttpClient, HttpConfig};
use reqwest_middleware::ClientWithMiddleware;
#[cfg(feature = "openai-whisper")]
use rig::providers::openai;
use serde::{Deserialize, Serialize};

#[cfg(feature = "openai-whisper")]
use crate::agent::AuthenticatedProvider;
use crate::agent::UnauthenticatedProvider;
use crate::error::Error;

/// Supported providers for speech-to-text transcription.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum SttProvider {
    /// OpenAI (Whisper)
    #[cfg(feature = "openai-whisper")]
    #[cfg_attr(docsrs, doc(cfg(feature = "openai-whisper")))]
    OpenAi(AuthenticatedProvider),
    /// Local speech-to-text provider (not yet implemented).
    Local(UnauthenticatedProvider),
}

impl SttProvider {
    /// Create an OpenAI transcription provider.
    #[cfg(feature = "openai-whisper")]
    #[cfg_attr(docsrs, doc(cfg(feature = "openai-whisper")))]
    pub fn openai(api_key: &str, model: &str) -> Self {
        Self::OpenAi(AuthenticatedProvider {
            api_key: api_key.to_owned(),
            model: model.to_owned(),
            base_url: None,
        })
    }

    /// Create a local transcription provider.
    ///
    /// **Not yet implemented** — calling [`SttService::transcribe`] with this
    /// provider will return an error.
    ///
    /// [`SttService::transcribe`]: super::SttService::transcribe
    pub fn local(model: &str) -> Self {
        Self::Local(UnauthenticatedProvider {
            model: model.to_owned(),
            base_url: None,
        })
    }

    /// Create a local transcription provider with a custom base URL.
    ///
    /// **Not yet implemented** — see [`local`].
    ///
    /// [`local`]: Self::local
    pub fn local_with_url(model: &str, url: &str) -> Self {
        Self::Local(UnauthenticatedProvider {
            model: model.to_owned(),
            base_url: Some(url.to_owned()),
        })
    }

    /// The model name for this provider.
    pub fn model(&self) -> &str {
        match self {
            #[cfg(feature = "openai-whisper")]
            Self::OpenAi(p) => &p.model,
            Self::Local(p) => &p.model,
        }
    }
}

/// Provider-erased dispatch enum for transcription models.
pub(crate) enum SttModels {
    #[cfg(feature = "openai-whisper")]
    OpenAi(openai::transcription::TranscriptionModel<ClientWithMiddleware>),
    Local,
}

impl SttModels {
    /// Build the appropriate transcription model for the given provider.
    pub fn from_provider(
        provider: &SttProvider,
        model: &str,
        max_retries: u32,
        client: Option<ClientWithMiddleware>,
    ) -> Result<Self, Error> {
        match provider {
            #[cfg(feature = "openai-whisper")]
            SttProvider::OpenAi(p) => {
                let http = match client {
                    Some(c) => c,
                    None => HttpClient::new(&HttpConfig {
                        max_retries,
                        ..HttpConfig::default()
                    })
                    .map_err(|e| Error::Request(e.to_string()))?
                    .into_inner(),
                };
                let client = p.openai_client(http)?;
                let model = openai::transcription::TranscriptionModel::new(client, model);
                Ok(Self::OpenAi(model))
            }
            SttProvider::Local(_) => {
                let _ = (model, max_retries, client);
                Ok(Self::Local)
            }
        }
    }
}
