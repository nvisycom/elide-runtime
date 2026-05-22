//! [`SttExtractor`]: STT-based audio transcription.
//!
//! Built once at engine startup from [`SttExtractorConfig`] and
//! shared across every run via [`Extractors`].
//!
//! [`Extractors`]: super::Extractors

mod params;

use nvisy_agent::audio::stt::SttService;
use nvisy_codec::ContentHandle;
use nvisy_codec::handler::{BoxedTextHandler, Handler, TxtHandler};
use nvisy_core::Result;
use nvisy_ontology::artifacts::{TranscriptSegment, Transcription};
use nvisy_ontology::primitive::TimeSpan;

pub use self::params::SttExtractorConfig;
use crate::operation::DocumentEnvelope;

const TARGET: &str = "nvisy_engine::extraction::stt";

/// Pre-built STT extractor: transcription service wrapping a provider.
pub struct SttExtractor {
    stt: SttService,
}

impl SttExtractor {
    /// Build from an [`SttExtractorConfig`] bundle.
    ///
    /// # Errors
    ///
    /// Returns an error if the STT service cannot be constructed.
    pub fn from_config(cfg: SttExtractorConfig) -> Result<Self> {
        let stt = SttService::new(&cfg.provider, cfg.agent)?;
        Ok(Self { stt })
    }

    /// Transcribe the envelope's audio (if it is audio) and replace
    /// the handle with the transcript text for downstream detection.
    ///
    /// `diarization` is currently advisory — diarization is not yet
    /// implemented; a warning is logged when requested.
    pub async fn run(&self, envelope: &mut DocumentEnvelope, diarization: bool) -> Result<()> {
        let ContentHandle::Audio(ref handler) = envelope.document.handle else {
            return Ok(());
        };

        if diarization {
            tracing::warn!(target: TARGET, "diarization not yet supported, skipping");
        }

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
            return Ok(());
        }

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

        let lines: Vec<String> = stt_result.text.lines().map(String::from).collect();
        let trailing = stt_result.text.ends_with('\n');
        let source = envelope.document.source();
        let handler = TxtHandler::new(lines, trailing).with_source(source);
        envelope.document.handle = ContentHandle::from(BoxedTextHandler::from(handler));
        tracing::debug!(target: TARGET, "replaced audio with transcript");
        Ok(())
    }
}
