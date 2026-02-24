//! Speech-to-text transcription action — generates text entities with audio
//! locations and transcript documents from audio input.

use serde::Deserialize;

use nvisy_codec::document::Document;
use nvisy_codec::handler::{Handler, WavHandler, TxtHandler};
use nvisy_core::Error;

use nvisy_ontology::entity::Entity;

pub use nvisy_asr::{TranscribeBackend, TranscribeConfig, parse_transcribe_entities};

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
    /// Detected entities with audio locations.
    pub entities: Vec<Entity>,
    /// Transcripts as new text documents.
    pub text_docs: Vec<Document<TxtHandler>>,
}

/// Speech-to-text action — delegates to a [`TranscribeBackend`] at runtime.
pub struct GenerateTranscribeAction<B> {
    backend: B,
    params: GenerateTranscribeParams,
}

impl<B: TranscribeBackend> GenerateTranscribeAction<B> {
    /// Create a new action with the given backend and params.
    pub fn new(backend: B, params: GenerateTranscribeParams) -> Self {
        Self { backend, params }
    }

    /// Build the [`TranscribeConfig`] from action parameters.
    fn config(&self) -> TranscribeConfig {
        TranscribeConfig {
            language: self.params.language.clone(),
            enable_speaker_diarization: self.params.enable_speaker_diarization,
            confidence_threshold: self.params.confidence_threshold,
        }
    }

    /// Execute transcription on audio documents.
    pub async fn run(&self, input: GenerateTranscribeInput) -> Result<GenerateTranscribeOutput, Error> {
        let config = self.config();
        let mut all_entities = Vec::new();
        let mut all_transcript_text = Vec::new();

        for doc in &input.audio_docs {
            let wav_bytes = doc.handler().encode()?;
            let raw = self
                .backend
                .transcribe(&wav_bytes, "audio/wav", &config)
                .await?;
            let entities = parse_transcribe_entities(&raw)?;
            for entity in &entities {
                all_transcript_text.push(entity.value.clone());
            }
            all_entities.extend(entities);
        }

        let mut text_docs = Vec::new();
        if !all_transcript_text.is_empty() {
            let text = all_transcript_text.join(" ");
            let handler = TxtHandler::new(
                text.lines().map(String::from).collect(),
                text.ends_with('\n'),
            );
            text_docs.push(Document::new(handler));
        }

        Ok(GenerateTranscribeOutput {
            entities: all_entities,
            text_docs,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nvisy_ontology::entity::DetectionMethod;
    use serde_json::{json, Value};

    #[test]
    fn parse_transcribe_entities_basic() {
        let raw = vec![json!({
            "text": "Hello world",
            "start_time": 0.5,
            "end_time": 1.2,
            "confidence": 0.95,
            "speaker_id": "speaker_1"
        })];
        let entities = parse_transcribe_entities(&raw).unwrap();
        assert_eq!(entities.len(), 1);
        assert_eq!(entities[0].value, "Hello world");
        assert_eq!(entities[0].detection_method, DetectionMethod::SpeechTranscript);

        let loc = entities[0].location.as_ref().unwrap().as_audio().unwrap();
        assert!((loc.time_span.start_secs - 0.5).abs() < f64::EPSILON);
        assert!((loc.time_span.end_secs - 1.2).abs() < f64::EPSILON);
        assert_eq!(loc.speaker_id.as_deref(), Some("speaker_1"));
    }

    struct MockTranscribeBackend;

    #[async_trait::async_trait]
    impl TranscribeBackend for MockTranscribeBackend {
        async fn transcribe(
            &self,
            _audio_data: &[u8],
            _mime_type: &str,
            _config: &TranscribeConfig,
        ) -> Result<Vec<Value>, Error> {
            Ok(vec![
                json!({
                    "text": "Hello",
                    "start_time": 0.0,
                    "end_time": 0.5,
                    "confidence": 0.9
                }),
                json!({
                    "text": "world",
                    "start_time": 0.5,
                    "end_time": 1.0,
                    "confidence": 0.85
                }),
            ])
        }
    }

    #[tokio::test]
    async fn run_produces_entities_and_text_docs() {
        use bytes::Bytes;

        let action = GenerateTranscribeAction::new(
            MockTranscribeBackend,
            GenerateTranscribeParams {
                language: "en".into(),
                enable_speaker_diarization: false,
                confidence_threshold: 0.5,
            },
        );

        let wav_handler = WavHandler::new(Bytes::from_static(b"fake-wav"));
        let input = GenerateTranscribeInput {
            audio_docs: vec![Document::new(wav_handler)],
        };

        let output = action.run(input).await.unwrap();
        assert_eq!(output.entities.len(), 2);
        assert_eq!(output.entities[0].value, "Hello");
        assert_eq!(output.entities[1].value, "world");
        assert_eq!(output.text_docs.len(), 1);
    }
}
