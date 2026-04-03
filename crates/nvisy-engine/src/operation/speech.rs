//! Audial extraction operation.
//!
//! Runs at **phase 1**, after ingestion. Transcribes speech audio into
//! text using automatic speech recognition.

use nvisy_codec::Document;
use nvisy_codec::handler::{BoxedTextHandler, Handler, TxtHandler};
use nvisy_core::{Error, ErrorKind, Result};
use nvisy_ontology::workflow::AudialExtraction as AudialExtractionCfg;
use nvisy_provider::audio::stt::{SttConfig, SttService};
use nvisy_provider::http::HttpClient;

use crate::operation::{DocumentEnvelope, Operation};
use crate::pipeline::RuntimeConfig;

const TARGET: &str = "nvisy_engine::op::speech";

/// Audial extraction: transcribes audio documents via STT.
pub struct AudialExtractionOp {
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
                    "audial_extraction requires an STT provider",
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
        if let Document::Audio(ref handler) = envelope.document {
            tracing::debug!(target: TARGET, "extracting audio for transcription");
            let audio_data = Handler::encode(handler)?;
            let filename = envelope
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
                tracing::debug!(target: TARGET, "transcription returned empty text, keeping original audio");
            } else {
                let lines: Vec<String> = stt_result.text.lines().map(String::from).collect();
                let trailing = stt_result.text.ends_with('\n');
                let source = envelope.document.source();
                let handler = TxtHandler::new(lines, trailing).with_source(source);
                envelope.document = Document::from(BoxedTextHandler::from(handler));
                tracing::debug!(target: TARGET, "replaced audio document with transcript text");
            }
        }
        Ok(())
    }
}
