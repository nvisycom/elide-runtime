//! Provider dispatch for text-to-speech models.

use reqwest_middleware::ClientWithMiddleware;
use rig::providers::openai;

use crate::backend::{AuthenticatedProvider, HttpConfig, build_http_client};
use crate::error::Error;

/// Supported providers for text-to-speech generation.
///
/// Currently only OpenAI supports TTS.
#[derive(Debug, Clone)]
pub enum TtsProvider {
    /// OpenAI (tts-1, tts-1-hd)
    OpenAi(AuthenticatedProvider),
}

impl TtsProvider {
    /// Create an OpenAI TTS provider.
    pub fn openai(api_key: &str, model: &str) -> Self {
        Self::OpenAi(AuthenticatedProvider {
            api_key: api_key.to_owned(),
            model: model.to_owned(),
            base_url: None,
        })
    }

    /// The model name for this provider.
    pub fn model(&self) -> &str {
        match self {
            Self::OpenAi(p) => &p.model,
        }
    }
}

/// Provider-erased dispatch enum for TTS models.
pub(crate) enum TtsModels {
    OpenAi(openai::audio_generation::AudioGenerationModel<ClientWithMiddleware>),
}

impl TtsModels {
    /// Build the appropriate TTS model for the given provider.
    pub fn from_provider(
        provider: &TtsProvider,
        model: &str,
        max_retries: u32,
    ) -> Result<Self, Error> {
        let http = build_http_client(&HttpConfig::with_max_retries(max_retries));

        match provider {
            TtsProvider::OpenAi(p) => {
                let client = p.openai_client(http)?;
                let model = openai::audio_generation::AudioGenerationModel::new(client, model);
                Ok(Self::OpenAi(model))
            }
        }
    }
}
