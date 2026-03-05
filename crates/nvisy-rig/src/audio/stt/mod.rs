//! Speech-to-text transcription service wrapping rig-core's `TranscriptionModel`.
//!
//! Not an LLM agent — directly calls the provider's transcription API (OpenAI
//! Whisper). Follows the same provider-dispatch enum pattern as
//! [`BaseAgent`](crate::agent::BaseAgent).

mod provider;

pub(crate) use provider::SttModels;
pub use provider::SttProvider;
#[cfg(feature = "openai")]
use rig::transcription::TranscriptionModel;
use uuid::Uuid;

use crate::error::Error;

/// Configuration for the speech-to-text service.
#[derive(Debug, Clone)]
pub struct SttConfig {
    /// Model name (e.g. `"whisper-1"`).
    pub model: String,
    /// BCP-47 language code (e.g. `"en"`, `"de"`).
    pub language: Option<String>,
    /// Sampling temperature for the transcription model.
    pub temperature: Option<f64>,
    /// Context hint / prompt for the transcription model.
    pub prompt: Option<String>,
    /// Maximum retries for transient HTTP errors (default: 3).
    pub max_retries: u32,
}

impl Default for SttConfig {
    fn default() -> Self {
        Self {
            model: "whisper-1".to_owned(),
            language: None,
            temperature: None,
            prompt: None,
            max_retries: 3,
        }
    }
}

/// Speech-to-text result.
#[derive(Debug, Clone)]
pub struct SttOutput {
    /// The transcribed text.
    pub text: String,
}

/// Speech-to-text service wrapping rig-core transcription providers.
///
/// Supports OpenAI (Whisper).
// TODO: Add diarization support once rig-core exposes verbose_json response
// format and timestamp_granularities options.
pub struct SttService {
    id: Uuid,
    inner: SttModels,
    config: SttConfig,
}

impl SttService {
    /// Create a new speech-to-text service for the given provider.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Request`] if client construction fails.
    pub fn new(provider: &SttProvider, config: SttConfig) -> Result<Self, Error> {
        let inner = SttModels::from_provider(provider, &config.model, config.max_retries)?;

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

    /// Transcribe audio data to text.
    ///
    /// # Arguments
    ///
    /// * `audio_data` — raw audio bytes (MP3, WAV, etc.).
    /// * `filename` — original filename, used for MIME-type detection.
    #[tracing::instrument(
        skip_all,
        fields(service_id = %self.id, data_len = audio_data.len(), filename),
    )]
    pub async fn transcribe(&self, audio_data: &[u8], filename: &str) -> Result<SttOutput, Error> {
        macro_rules! build_and_send {
            ($model:expr) => {{
                let mut builder = $model
                    .transcription_request()
                    .data(audio_data.to_vec())
                    .filename(Some(filename.to_owned()));

                if let Some(ref lang) = self.config.language {
                    builder = builder.language(lang.clone());
                }
                if let Some(temp) = self.config.temperature {
                    builder = builder.temperature(temp);
                }
                if let Some(ref prompt) = self.config.prompt {
                    builder = builder.prompt(prompt.clone());
                }

                builder.send().await?.text
            }};
        }

        let text: String = match &self.inner {
            #[cfg(feature = "openai")]
            SttModels::OpenAi(model) => build_and_send!(model),
            SttModels::Local => {
                return Err(Error::Runtime(
                    "local speech-to-text provider is not yet implemented".to_owned(),
                ));
            }
        };

        tracing::info!(text_len = text.len(), "transcription complete");

        Ok(SttOutput { text })
    }
}
