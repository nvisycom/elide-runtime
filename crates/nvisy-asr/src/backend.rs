//! Transcription backend trait and configuration.

use serde_json::Value;

use nvisy_core::Error;

/// Configuration passed to a [`TranscribeBackend`] implementation.
#[derive(Debug, Clone)]
pub struct TranscribeConfig {
    /// BCP-47 language tag for transcription.
    pub language: String,
    /// Whether to perform speaker diarization.
    pub enable_speaker_diarization: bool,
    /// Minimum confidence threshold for results.
    pub confidence_threshold: f64,
}

/// Backend trait for transcription providers.
///
/// Implementations call an external speech-to-text service and return
/// raw JSON results.  Entity construction is handled by the consuming crate.
#[async_trait::async_trait]
pub trait TranscribeBackend: Send + Sync + 'static {
    /// Transcribe audio bytes, returning raw dicts.
    ///
    /// Each dict should contain: `text`, `start_time`, `end_time`, `confidence`,
    /// and optionally `speaker_id`.
    async fn transcribe(
        &self,
        audio_data: &[u8],
        mime_type: &str,
        config: &TranscribeConfig,
    ) -> Result<Vec<Value>, Error>;
}
