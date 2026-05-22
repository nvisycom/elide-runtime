//! Text-to-speech generation service wrapping rig-core's `AudioGenerationModel`.

mod provider;

use nvisy_core::{Error, Result};
#[cfg(feature = "openai-tts")]
use rig::audio_generation::AudioGenerationModel as _;
use uuid::Uuid;

pub(crate) use self::provider::TtsModels;
pub use self::provider::TtsProvider;

#[cfg(feature = "openai-tts")]
const TARGET: &str = "nvisy_rig::tts";

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
    #[allow(dead_code)]
    config: TtsConfig,
}

impl TtsService {
    /// Create a new TTS service for the given provider. The HTTP
    /// client is built internally from `config.max_retries`.
    ///
    /// # Errors
    ///
    /// Returns an [`Error`] with [`ErrorKind::Validation`] if client
    /// construction fails.
    ///
    /// [`ErrorKind::Validation`]: nvisy_core::ErrorKind::Validation
    pub fn new(provider: &TtsProvider, config: TtsConfig) -> Result<Self> {
        let inner = TtsModels::from_provider(provider, &config.model, config.max_retries, None)
            .map_err(crate::error::convert)?;

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
        target = "nvisy_rig::tts",
        skip_all,
        fields(service_id = %self.id, text_len = text.len()),
    )]
    pub async fn generate(&self, text: &str) -> Result<Vec<u8>> {
        match &self.inner {
            #[cfg(feature = "openai-tts")]
            TtsModels::OpenAi(model) => {
                let response = model
                    .audio_generation_request()
                    .text(text)
                    .voice(&self.config.voice)
                    .speed(self.config.speed)
                    .send()
                    .await
                    .map_err(crate::error::convert)?;
                tracing::info!(target: TARGET, audio_len = response.audio.len(), "audio generation complete");
                Ok(response.audio)
            }
            TtsModels::Local => Err(Error::runtime(
                "local text-to-speech provider is not yet implemented",
                "provider",
                false,
            )),
        }
    }
}
