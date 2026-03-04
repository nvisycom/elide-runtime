//! Provider-erased dispatch enums and constructors for audio models.

use rig::providers::{gemini, openai};

use reqwest_middleware::ClientWithMiddleware;

use crate::backend::{AuthenticatedProvider, HttpConfig, build_http_client};
use crate::error::Error;

/// Supported providers for speech-to-text transcription.
///
/// Only OpenAI (Whisper) and Gemini support transcription.
#[derive(Debug, Clone)]
pub enum TranscribeProvider {
    /// OpenAI (Whisper)
    OpenAi(AuthenticatedProvider),
    /// Google Gemini
    Gemini(AuthenticatedProvider),
}

impl TranscribeProvider {
    /// Create an OpenAI transcription provider.
    pub fn openai(api_key: &str, model: &str) -> Self {
        Self::OpenAi(AuthenticatedProvider {
            api_key: api_key.to_owned(),
            model: model.to_owned(),
            base_url: None,
        })
    }

    /// Create a Gemini transcription provider.
    pub fn gemini(api_key: &str, model: &str) -> Self {
        Self::Gemini(AuthenticatedProvider {
            api_key: api_key.to_owned(),
            model: model.to_owned(),
            base_url: None,
        })
    }

    /// The model name for this provider.
    pub fn model(&self) -> &str {
        match self {
            Self::OpenAi(p) | Self::Gemini(p) => &p.model,
        }
    }
}

/// Provider-erased dispatch enum for transcription models.
pub(crate) enum TranscribeModels {
    OpenAi(openai::transcription::TranscriptionModel<ClientWithMiddleware>),
    Gemini(gemini::transcription::TranscriptionModel<ClientWithMiddleware>),
}

impl TranscribeModels {
    /// Build the appropriate transcription model for the given provider.
    pub fn from_provider(
        provider: &TranscribeProvider,
        model: &str,
        max_retries: u32,
    ) -> Result<Self, Error> {
        let http = build_http_client(&HttpConfig::with_max_retries(max_retries));

        match provider {
            TranscribeProvider::OpenAi(p) => {
                let client = p.openai_client(http)?;
                let model =
                    openai::transcription::TranscriptionModel::new(client, model);
                Ok(Self::OpenAi(model))
            }
            TranscribeProvider::Gemini(p) => {
                let client = p.gemini_client(http)?;
                // rig-core 0.31: Gemini's Capabilities doesn't propagate H,
                // so TranscriptionClient is unavailable for non-default H.
                let model =
                    gemini::transcription::TranscriptionModel::new(client, model);
                Ok(Self::Gemini(model))
            }
        }
    }
}

/// Supported providers for audio generation (TTS).
///
/// Currently only OpenAI supports TTS.
#[cfg(feature = "audio")]
#[derive(Debug, Clone)]
pub enum AudioGenProvider {
    /// OpenAI (tts-1, tts-1-hd)
    OpenAi(AuthenticatedProvider),
}

#[cfg(feature = "audio")]
impl AudioGenProvider {
    /// Create an OpenAI audio generation provider.
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

/// Provider-erased dispatch enum for audio generation (TTS) models.
#[cfg(feature = "audio")]
pub(crate) enum AudioGenModels {
    OpenAi(openai::audio_generation::AudioGenerationModel<ClientWithMiddleware>),
}

#[cfg(feature = "audio")]
impl AudioGenModels {
    /// Build the appropriate audio generation model for the given provider.
    pub fn from_provider(
        provider: &AudioGenProvider,
        model: &str,
        max_retries: u32,
    ) -> Result<Self, Error> {
        let http = build_http_client(&HttpConfig::with_max_retries(max_retries));

        match provider {
            AudioGenProvider::OpenAi(p) => {
                let client = p.openai_client(http)?;
                let model =
                    openai::audio_generation::AudioGenerationModel::new(client, model);
                Ok(Self::OpenAi(model))
            }
        }
    }
}
