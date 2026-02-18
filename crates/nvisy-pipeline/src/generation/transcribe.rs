//! Speech-to-text transcription action — generates text entities with audio
//! locations and transcript documents from audio input.

use serde::Deserialize;

use nvisy_codec::document::Document;
use nvisy_codec::handler::{WavHandler, TxtHandler};
use crate::ontology::Entity;
use nvisy_core::error::Error;

fn default_language() -> String {
    "en".into()
}

fn default_confidence() -> f64 {
    0.5
}

/// Typed parameters for [`GenerateTranscribeAction`].
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerateTranscribeParams {
    /// BCP-47 language tag for transcription.
    #[serde(default = "default_language")]
    pub language: String,
    /// Whether to perform speaker diarization.
    #[serde(default)]
    pub enable_speaker_diarization: bool,
    /// Minimum confidence score for returned entities.
    #[serde(default = "default_confidence")]
    pub confidence_threshold: f64,
}

/// Typed input for [`GenerateTranscribeAction`].
pub struct GenerateTranscribeInput {
    /// Audio documents to transcribe.
    pub audio_docs: Vec<Document<WavHandler>>,
}

/// Typed output for [`GenerateTranscribeAction`].
pub struct GenerateTranscribeOutput {
    /// Detected entities with [`AudioLocation`](crate::ontology::entity::AudioLocation).
    pub entities: Vec<Entity>,
    /// Transcripts as new text documents.
    pub text_docs: Vec<Document<TxtHandler>>,
}

/// Speech-to-text stub — delegates to a transcription provider at runtime.
pub struct GenerateTranscribeAction;

impl GenerateTranscribeAction {
    pub async fn connect(_params: GenerateTranscribeParams) -> Result<Self, Error> {
        Ok(Self)
    }

    pub async fn execute(
        &self,
        _input: GenerateTranscribeInput,
    ) -> Result<GenerateTranscribeOutput, Error> {
        // Stub: real implementation will call a speech-to-text provider.
        Ok(GenerateTranscribeOutput {
            entities: Vec::new(),
            text_docs: Vec::new(),
        })
    }
}
