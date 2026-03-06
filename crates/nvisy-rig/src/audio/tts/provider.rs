//! Provider dispatch for text-to-speech models.

#[cfg(feature = "openai-tts")]
use reqwest_middleware::ClientWithMiddleware;
#[cfg(feature = "openai-tts")]
use rig::providers::openai;
use serde::{Deserialize, Serialize};

#[cfg(feature = "openai-tts")]
use crate::backend::{AuthenticatedProvider, HttpConfig, build_http_client};
use crate::backend::UnauthenticatedProvider;
use crate::error::Error;

/// Supported providers for text-to-speech generation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TtsProvider {
    /// OpenAI (tts-1, tts-1-hd)
    #[cfg(feature = "openai-tts")]
    #[cfg_attr(docsrs, doc(cfg(feature = "openai-tts")))]
    OpenAi(AuthenticatedProvider),
    /// Local text-to-speech provider (not yet implemented).
    Local(UnauthenticatedProvider),
}

impl TtsProvider {
    /// Create an OpenAI TTS provider.
    #[cfg(feature = "openai-tts")]
    #[cfg_attr(docsrs, doc(cfg(feature = "openai-tts")))]
    pub fn openai(api_key: &str, model: &str) -> Self {
        Self::OpenAi(AuthenticatedProvider {
            api_key: api_key.to_owned(),
            model: model.to_owned(),
            base_url: None,
        })
    }

    /// Create a local TTS provider.
    ///
    /// **Not yet implemented** — calling [`TtsService::generate`] with this
    /// provider will return an error.
    ///
    /// [`TtsService::generate`]: super::TtsService::generate
    pub fn local(model: &str) -> Self {
        Self::Local(UnauthenticatedProvider {
            model: model.to_owned(),
            base_url: None,
        })
    }

    /// Create a local TTS provider with a custom base URL.
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
            #[cfg(feature = "openai-tts")]
            Self::OpenAi(p) => &p.model,
            Self::Local(p) => &p.model,
        }
    }
}

/// Provider-erased dispatch enum for TTS models.
pub(crate) enum TtsModels {
    #[cfg(feature = "openai-tts")]
    OpenAi(openai::audio_generation::AudioGenerationModel<ClientWithMiddleware>),
    Local,
}

impl TtsModels {
    /// Build the appropriate TTS model for the given provider.
    pub fn from_provider(
        provider: &TtsProvider,
        model: &str,
        max_retries: u32,
    ) -> Result<Self, Error> {
        match provider {
            #[cfg(feature = "openai-tts")]
            TtsProvider::OpenAi(p) => {
                let http = build_http_client(&HttpConfig::with_max_retries(max_retries));
                let client = p.openai_client(http)?;
                let model = openai::audio_generation::AudioGenerationModel::new(client, model);
                Ok(Self::OpenAi(model))
            }
            TtsProvider::Local(_) => Ok(Self::Local),
        }
    }
}
