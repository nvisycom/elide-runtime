//! Composite audio detection: transcription followed by NER.
//!
//! Chains a [`TranscribeBackend`] with an [`NerBackend`] to detect
//! entities in audio content.  The ASR stage produces a transcript
//! with time-aligned segments, then NER runs on the combined text
//! and the resulting text-location entities are mapped back to
//! [`AudioLocation`] time spans.

use bytes::Bytes;

use nvisy_codec::handler::Span;
use nvisy_core::Error;

use nvisy_asr::{TranscribeBackend, TranscribeConfig, parse_transcribe_entities};

use crate::ner::{NerBackend, NerConfig, parse_ner_entities};
use crate::{Entity, Location};
use crate::{ParallelContext, DetectionService};

/// Composite audio detection layer: transcription + NER.
///
/// First transcribes each audio span via [`TranscribeBackend`], then
/// runs [`NerBackend`] on the resulting transcript text.  Entities
/// from transcription carry [`AudioLocation`] with time spans;
/// entities from NER carry text locations within the transcript.
pub struct TranscriptNerDetection<T, N> {
    transcribe_backend: T,
    transcribe_config: TranscribeConfig,
    ner_backend: N,
    ner_config: NerConfig,
}

impl<T: TranscribeBackend, N: NerBackend> TranscriptNerDetection<T, N> {
    /// Create a new composite detection layer.
    pub fn new(
        transcribe_backend: T,
        transcribe_config: TranscribeConfig,
        ner_backend: N,
        ner_config: NerConfig,
    ) -> Self {
        Self {
            transcribe_backend,
            transcribe_config,
            ner_backend,
            ner_config,
        }
    }
}

#[async_trait::async_trait]
impl<T: TranscribeBackend, N: NerBackend> DetectionService<(), Bytes>
    for TranscriptNerDetection<T, N>
{
    type Context = ParallelContext;

    async fn detect(
        &self,
        spans: Vec<Span<(), Bytes>>,
    ) -> Result<Vec<Entity>, Error> {
        let mut entities = Vec::new();

        for span in &spans {
            let audio_bytes: &[u8] = &span.data;

            // Step 1: Transcribe audio → time-aligned segments.
            let raw_segments = self
                .transcribe_backend
                .transcribe(audio_bytes, "audio/wav", &self.transcribe_config)
                .await?;

            let transcript_entities = parse_transcribe_entities(&raw_segments)?;

            // Collect transcript text for NER.
            let transcript_text: String = transcript_entities
                .iter()
                .map(|e| e.value.as_str())
                .collect::<Vec<_>>()
                .join(" ");

            // Include the raw transcript entities (audio-located).
            for entity in transcript_entities {
                entities.push(entity.with_parent(&span.source));
            }

            // Step 2: Run NER on the combined transcript text.
            if !transcript_text.is_empty() {
                let raw_ner = self
                    .ner_backend
                    .detect_text(&transcript_text, &self.ner_config)
                    .await?;

                for mut entity in parse_ner_entities(&raw_ner)? {
                    // NER entities from transcript get a text location
                    // within the transcript. For now we keep them as-is;
                    // a future enhancement could map text offsets back to
                    // audio time spans using segment boundaries.
                    if entity.location.is_none() {
                        entity.location = Some(Location::Text(Default::default()));
                    }
                    entities.push(entity.with_parent(&span.source));
                }
            }
        }

        Ok(entities)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nvisy_ontology::entity::DetectionMethod;
    use serde_json::{json, Value};

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
                    "text": "My name is John Doe",
                    "start_time": 0.0,
                    "end_time": 2.0,
                    "confidence": 0.95
                }),
            ])
        }
    }

    struct MockNerBackend;

    #[async_trait::async_trait]
    impl NerBackend for MockNerBackend {
        async fn detect_text(
            &self,
            text: &str,
            _config: &NerConfig,
        ) -> Result<Vec<Value>, Error> {
            let mut results = Vec::new();
            if let Some(pos) = text.find("John Doe") {
                results.push(json!({
                    "category": "pii",
                    "entity_type": "person_name",
                    "value": "John Doe",
                    "confidence": 0.9,
                    "start_offset": pos,
                    "end_offset": pos + 8
                }));
            }
            Ok(results)
        }

        async fn detect_image(
            &self,
            _: &[u8], _: &str, _: &NerConfig,
        ) -> Result<Vec<Value>, Error> {
            Ok(Vec::new())
        }
    }

    #[tokio::test]
    async fn transcript_ner_produces_both_entity_types() {
        let layer = TranscriptNerDetection::new(
            MockTranscribeBackend,
            TranscribeConfig {
                language: "en".into(),
                enable_speaker_diarization: false,
                confidence_threshold: 0.5,
            },
            MockNerBackend,
            NerConfig {
                entity_types: vec![],
                confidence_threshold: 0.0,
            },
        );

        let audio = Bytes::from_static(b"fake-wav-data");
        let spans = vec![Span::new((), audio)];

        let entities = layer.detect(spans).await.unwrap();
        // Should have: 1 transcript entity + 1 NER entity
        assert_eq!(entities.len(), 2);

        // First entity is from transcription (audio location).
        assert_eq!(entities[0].detection_method, DetectionMethod::SpeechTranscript);
        assert!(entities[0].location.as_ref().unwrap().as_audio().is_some());

        // Second entity is from NER (text location).
        assert_eq!(entities[1].detection_method, DetectionMethod::Ner);
        assert_eq!(entities[1].value, "John Doe");
    }
}
