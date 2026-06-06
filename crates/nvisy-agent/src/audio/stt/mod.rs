//! Speech-to-text transcription service wrapping rig-core's `TranscriptionModel`.
//!
//! Not an LLM agent — directly calls the provider's transcription API (OpenAI
//! Whisper). Follows the same provider-dispatch enum pattern as
//! `BaseAgent`.

mod provider;

use nvisy_core::entity::ModelProvenance;
use nvisy_core::extraction::{Extractor, ExtractorOutput, Span};
use nvisy_core::modality::{Audio, AudioExtraction};
use nvisy_core::{Error, Result};
#[cfg(feature = "openai-whisper")]
use rig::transcription::TranscriptionModel;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub(crate) use self::provider::SttModels;
pub use self::provider::SttProvider;

#[cfg(feature = "openai-whisper")]
const TARGET: &str = "nvisy_agent::stt";

/// Configuration for the speech-to-text service.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
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
    /// The HTTP client is built internally from `config.max_retries`.
    ///
    /// # Errors
    ///
    /// Returns an [`Error`] with [`ErrorKind::Validation`] if client
    /// construction fails.
    ///
    /// [`ErrorKind::Validation`]: nvisy_core::ErrorKind::Validation
    pub fn new(provider: &SttProvider, config: SttConfig) -> Result<Self> {
        let inner = SttModels::from_provider(provider, &config.model, config.max_retries, None)
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

    /// Provenance of the underlying transcription model.
    pub fn provenance(&self) -> ModelProvenance {
        ModelProvenance::new(self.config.model.clone())
    }

    /// Transcribe audio data to text.
    ///
    /// # Arguments
    ///
    /// * `audio_data` — raw audio bytes (MP3, WAV, etc.).
    /// * `filename` — original filename, used for MIME-type detection.
    #[tracing::instrument(
        target = "nvisy_agent::stt",
        skip_all,
        fields(service_id = %self.id, data_len = audio_data.len(), filename),
    )]
    #[allow(unused_variables)]
    pub async fn transcribe(&self, audio_data: &[u8], filename: &str) -> Result<SttOutput> {
        match &self.inner {
            #[cfg(feature = "openai-whisper")]
            SttModels::OpenAi(model) => {
                let mut builder = model
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

                let text = builder.send().await.map_err(crate::error::convert)?.text;
                tracing::info!(target: TARGET, text_len = text.len(), "transcription complete");
                Ok(SttOutput { text })
            }
            SttModels::Local => Err(Error::runtime(
                "local speech-to-text provider is not yet implemented",
                "provider",
                false,
            )),
        }
    }
}

#[async_trait::async_trait]
impl Extractor<Audio> for SttService {
    type Output = SttOutput;

    async fn extract(&self, span: &Span<Audio>) -> Result<ExtractorOutput<Audio, Self::Output>> {
        let synthesized;
        let filename = match span.data.filename.as_deref() {
            Some(name) => name,
            None => {
                synthesized = format!("audio.{}", span.data.extension());
                synthesized.as_str()
            }
        };
        let value = self.transcribe(span.data.bytes.as_ref(), filename).await?;
        Ok(ExtractorOutput::new(
            value,
            AudioExtraction::Transcription(self.provenance()),
        ))
    }
}
