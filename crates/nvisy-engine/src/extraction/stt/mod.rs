//! [`SttExtractor`]: STT-based audio transcription.
//!
//! Built once at engine startup from [`SttExtractorConfig`] and
//! shared across every run via [`Extractors`].
//!
//! [`Extractors`]: super::Extractors

mod params;

use nvisy_agent::audio::stt::SttService;
use nvisy_codec::DocumentHandle;
use nvisy_codec::handler::{BoxedTextHandler, Handler};
use nvisy_core::Result;
use nvisy_formats::text::TxtHandler;
use nvisy_ontology::document::{Block, Document};
use nvisy_ontology::modality::{Audio, AudioBlockKind, AudioMetadata};
use nvisy_ontology::primitive::TimeSpan;

pub use self::params::SttExtractorConfig;
use crate::envelope::DocumentEnvelope;

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

    /// Transcribe the envelope's audio (if it is audio) into
    /// [`DocumentEnvelope::audio`] and replace the codec handle with
    /// the transcript text so downstream text detection can run.
    ///
    /// `diarization` is currently advisory — diarization is not yet
    /// implemented; a warning is logged when requested.
    pub async fn run(&self, envelope: &mut DocumentEnvelope, diarization: bool) -> Result<()> {
        let DocumentHandle::Audio(ref handler) = envelope.handle else {
            return Ok(());
        };

        if diarization {
            tracing::warn!(target: TARGET, "diarization not yet supported, skipping");
        }

        tracing::debug!(target: TARGET, "transcribing audio");
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
            tracing::debug!(target: TARGET, "transcription returned empty text");
            return Ok(());
        }

        // Record the transcript as a single-block Document<Audio>.
        // Real time-span / speaker diarization will populate finer
        // structure once the upstream service exposes it.
        let location = Audio::new(TimeSpan::new(0, 0));
        envelope.audio = Some(Document::new(
            AudioMetadata::default(),
            vec![Block {
                text: stt_result.text.clone(),
                spans: Vec::new(),
                kind: AudioBlockKind::Speech,
                confidence: None,
                source: location,
                artefacts: Vec::new(),
            }],
        ));

        let lines: Vec<String> = stt_result.text.lines().map(String::from).collect();
        let trailing = stt_result.text.ends_with('\n');
        let source = envelope.source();
        let handler = TxtHandler::new(lines, trailing).with_source(source);
        envelope.handle = DocumentHandle::from(BoxedTextHandler::new(handler));
        tracing::debug!(target: TARGET, "replaced audio with transcript");
        Ok(())
    }
}
