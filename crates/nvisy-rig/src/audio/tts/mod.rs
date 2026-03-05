//! Text-to-speech generation service wrapping rig-core's `AudioGenerationModel`.

mod provider;

pub(crate) use provider::TtsModels;
pub use provider::TtsProvider;
use rig::audio_generation::AudioGenerationModel as _;
use uuid::Uuid;

use crate::error::Error;

/// Configuration for the text-to-speech service.
#[derive(Debug, Clone)]
pub struct TtsConfig {
    /// Model name (e.g. `"tts-1"`, `"tts-1-hd"`).
    pub model: String,
    /// Voice name (e.g. `"alloy"`, `"nova"`, `"shimmer"`).
    pub voice: String,
    /// Playback speed multiplier (default: 1.0).
    pub speed: f32,
    /// Maximum retries for transient HTTP errors (default: 3).
    pub max_retries: u32,
}

impl Default for TtsConfig {
    fn default() -> Self {
        Self {
            model: "tts-1".to_owned(),
            voice: "alloy".to_owned(),
            speed: 1.0,
            max_retries: 3,
        }
    }
}

/// Text-to-speech generation service wrapping rig-core audio generation providers.
///
/// Currently only supports OpenAI (tts-1, tts-1-hd).
pub struct TtsService {
    id: Uuid,
    inner: TtsModels,
    config: TtsConfig,
}

impl TtsService {
    /// Create a new TTS service for the given provider.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Request`] if client construction fails.
    pub fn new(provider: &TtsProvider, config: TtsConfig) -> Result<Self, Error> {
        let inner = TtsModels::from_provider(provider, &config.model, config.max_retries)?;

        Ok(Self {
            id: Uuid::now_v7(),
            inner,
            config,
        })
    }

    /// Unique identifier for this service instance (UUIDv7).
    pub fn id(&self) -> Uuid {
        self.id
    }

    /// Generate speech from text, returning raw audio bytes.
    #[tracing::instrument(
        skip_all,
        fields(service_id = %self.id, text_len = text.len()),
    )]
    pub async fn generate(&self, text: &str) -> Result<Vec<u8>, Error> {
        let audio = match &self.inner {
            TtsModels::OpenAi(model) => {
                let response = model
                    .audio_generation_request()
                    .text(text)
                    .voice(&self.config.voice)
                    .speed(self.config.speed)
                    .send()
                    .await?;
                response.audio
            }
        };

        tracing::info!(audio_len = audio.len(), "audio generation complete");

        Ok(audio)
    }
}
