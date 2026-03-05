//! Provider dispatch for speech-to-text models.

use rig::providers::openai;
use reqwest_middleware::ClientWithMiddleware;

use crate::backend::{AuthenticatedProvider, HttpConfig, build_http_client};
use crate::error::Error;

/// Supported providers for speech-to-text transcription.
///
/// Only OpenAI (Whisper) supports transcription.
#[derive(Debug, Clone)]
pub enum SttProvider {
    /// OpenAI (Whisper)
    OpenAi(AuthenticatedProvider),
}

impl SttProvider {
    /// Create an OpenAI transcription provider.
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

/// Provider-erased dispatch enum for transcription models.
pub(crate) enum SttModels {
    OpenAi(openai::transcription::TranscriptionModel<ClientWithMiddleware>),
}

impl SttModels {
    /// Build the appropriate transcription model for the given provider.
    pub fn from_provider(
        provider: &SttProvider,
        model: &str,
        max_retries: u32,
    ) -> Result<Self, Error> {
        let http = build_http_client(&HttpConfig::with_max_retries(max_retries));

        match provider {
            SttProvider::OpenAi(p) => {
                let client = p.openai_client(http)?;
                let model =
                    openai::transcription::TranscriptionModel::new(client, model);
                Ok(Self::OpenAi(model))
            }
        }
    }
}
