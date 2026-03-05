//! Provider dispatch for speech-to-text models.

#[cfg(feature = "openai")]
use reqwest_middleware::ClientWithMiddleware;
#[cfg(feature = "openai")]
use rig::providers::openai;
use serde::{Deserialize, Serialize};

#[cfg(feature = "openai")]
use crate::backend::{AuthenticatedProvider, HttpConfig, build_http_client};
use crate::backend::UnauthenticatedProvider;
use crate::error::Error;

/// Supported providers for speech-to-text transcription.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SttProvider {
    /// OpenAI (Whisper)
    #[cfg(feature = "openai")]
    #[cfg_attr(docsrs, doc(cfg(feature = "openai")))]
    OpenAi(AuthenticatedProvider),
    /// Local speech-to-text provider (not yet implemented).
    Local(UnauthenticatedProvider),
}

impl SttProvider {
    /// Create an OpenAI transcription provider.
    #[cfg(feature = "openai")]
    #[cfg_attr(docsrs, doc(cfg(feature = "openai")))]
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
    /// **Not yet implemented** — see [`local`](Self::local).
    pub fn local_with_url(model: &str, url: &str) -> Self {
        Self::Local(UnauthenticatedProvider {
            model: model.to_owned(),
            base_url: Some(url.to_owned()),
        })
    }

    /// The model name for this provider.
    pub fn model(&self) -> &str {
        match self {
            #[cfg(feature = "openai")]
            Self::OpenAi(p) => &p.model,
            Self::Local(p) => &p.model,
        }
    }
}

/// Provider-erased dispatch enum for transcription models.
pub(crate) enum SttModels {
    #[cfg(feature = "openai")]
    OpenAi(openai::transcription::TranscriptionModel<ClientWithMiddleware>),
    Local,
}

impl SttModels {
    /// Build the appropriate transcription model for the given provider.
    pub fn from_provider(
        provider: &SttProvider,
        model: &str,
        max_retries: u32,
    ) -> Result<Self, Error> {
        match provider {
            #[cfg(feature = "openai")]
            SttProvider::OpenAi(p) => {
                let http = build_http_client(&HttpConfig::with_max_retries(max_retries));
                let client = p.openai_client(http)?;
                let model = openai::transcription::TranscriptionModel::new(client, model);
                Ok(Self::OpenAi(model))
            }
            SttProvider::Local(_) => Ok(Self::Local),
        }
    }
}
