//! [`SttExtractor`]: STT-based audio transcription.
//!
//! Built once at engine startup from [`SttExtractorConfig`] and
//! shared across every run via [`Extractors`].
//!
//! [`Extractors`]: super::Extractors

mod params;

use nvisy_agent::audio::stt::SttService;
use nvisy_codec::DocumentHandle;
use nvisy_core::Result;
use nvisy_ontology::document::Block;
use nvisy_ontology::modality::{Audio, AudioBlock, AudioExtraction};
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

    /// Transcribe the envelope's audio into
    /// [`DocumentEnvelope::document`]. The handle stays as audio —
    /// downstream text detection runs against a separate text
    /// envelope spawned by the pipeline orchestrator.
    ///
    /// `diarization` is currently advisory — diarization is not yet
    /// implemented; a warning is logged when requested.
    pub async fn run(
        &self,
        envelope: &mut DocumentEnvelope<Audio>,
        diarization: bool,
    ) -> Result<()> {
        // Stamp the real provenance over the importer's placeholder
        // ahead of any early returns — even an empty transcript
        // should reflect the model that ran.
        let provenance = self.stt.provenance();
        envelope.document.meta.extraction = if diarization {
            AudioExtraction::Diarization(provenance)
        } else {
            AudioExtraction::Transcription(provenance)
        };

        let audio_data = {
            let handle = envelope.handle.lock().await;
            let DocumentHandle::Audio(ref handler) = *handle else {
                return Ok(());
            };
            handler.encode()?
        };

        if diarization {
            tracing::warn!(target: TARGET, "diarization not yet supported, skipping");
        }

        tracing::debug!(target: TARGET, "transcribing audio");
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

        let time_span = TimeSpan::new(0, 0);
        envelope
            .document
            .blocks
            .push(Block::new(AudioBlock::Speech {
                time_span,
                text: stt_result.text.clone(),
                speaker_id: None,
            }));

        tracing::debug!(target: TARGET, "audio transcript captured");
        Ok(())
    }
}
