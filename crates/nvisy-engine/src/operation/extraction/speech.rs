//! Audial extraction operation.
//!
//! Transcribes speech audio into text using automatic speech recognition.

use nvisy_codec::ContentHandle;
use nvisy_codec::handler::{BoxedTextHandler, Handler, TxtHandler};
use nvisy_core::{Error, ErrorKind, Result};
use nvisy_ontology::artifacts::{TranscriptSegment, Transcription};
use nvisy_ontology::primitive::TimeSpan;
use nvisy_ontology::workflow::AudialExtraction as AudialExtractionCfg;
use nvisy_provider::audio::stt::{SttConfig, SttService};
use nvisy_provider::http::HttpClient;

use crate::operation::{DocumentEnvelope, Operation};
use crate::pipeline::RuntimeConfig;

const TARGET: &str = "nvisy_engine::op::extraction::audial";

/// Audial extraction: transcribes audio documents via STT.
pub(super) struct AudialExtractionOp {
    stt: SttService,
}

impl AudialExtractionOp {
    pub fn new(
        cfg: &AudialExtractionCfg,
        config: &RuntimeConfig,
        http_client: &HttpClient,
    ) -> Result<Self> {
        let stt_provider = config
            .stt
            .as_ref()
            .and_then(|s| s.provider.clone())
            .ok_or_else(|| {
                Error::new(
                    ErrorKind::Validation,
                    "audial extraction requires an STT provider",
                )
            })?;

        let stt = SttService::new(
            &stt_provider,
            SttConfig::default(),
            Some(http_client.clone()),
        )?;

        if cfg.diarization {
            tracing::warn!(target: TARGET, "diarization not yet supported, skipping");
        }

        Ok(Self { stt })
    }
}

impl Operation for AudialExtractionOp {
    async fn execute(&self, envelope: &mut DocumentEnvelope) -> Result<()> {
        if let ContentHandle::Audio(ref handler) = envelope.document.handle {
            tracing::debug!(target: TARGET, "transcribing audio");
            let audio_data = Handler::encode(handler)?;
            let filename = envelope
                .document
                .metadata
                .filename
                .as_deref()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|| "audio.wav".to_string());

            let stt_result = self
                .stt
                .transcribe(audio_data.as_bytes(), &filename)
                .await?;

            if stt_result.text.is_empty() {
                tracing::debug!(target: TARGET, "transcription returned empty text");
            } else {
                // Store transcription in artifacts.
                // TODO: populate real segment timestamps once the STT provider
                // returns verbose_json with timestamp_granularities.
                if let Some(audio) = envelope.document.artifacts.as_audio_mut() {
                    audio.transcription = Some(Transcription {
                        segments: vec![TranscriptSegment {
                            text: stt_result.text.clone(),
                            time_span: TimeSpan::new(0, 0),
                            speaker_id: None,
                            confidence: None,
                        }],
                        language: None,
                    });
                }

                // Replace audio handle with text for downstream detection.
                let lines: Vec<String> = stt_result.text.lines().map(String::from).collect();
                let trailing = stt_result.text.ends_with('\n');
                let source = envelope.document.source();
                let handler = TxtHandler::new(lines, trailing).with_source(source);
                envelope.document.handle = ContentHandle::from(BoxedTextHandler::from(handler));
                tracing::debug!(target: TARGET, "replaced audio with transcript");
            }
        }
        Ok(())
    }
}
