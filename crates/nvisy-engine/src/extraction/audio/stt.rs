//! [`SttExtractor`]: STT-based audio transcription.
//!
//! Built once at engine startup from [`SttExtractorConfig`] and
//! shared across every run via [`ExtractionEngine`].
//!
//! [`ExtractionEngine`]: super::super::ExtractionEngine

use nvisy_agent::audio::SttProvider;
use nvisy_agent::audio::stt::{SttConfig, SttService};
use nvisy_codec::DocumentHandle;
use nvisy_core::Result;
use nvisy_ontology::document::Block;
use nvisy_ontology::modality::{Audio, AudioBlock, AudioExtraction};
use nvisy_ontology::primitive::TimeSpan;
use serde::{Deserialize, Serialize};

const TARGET: &str = "nvisy_engine::extraction::audio::stt";

/// `[extractor.stt]` config bundle.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SttExtractorConfig {
    /// Enable this extractor. When `false`, the extractor is
    /// neither built nor dispatched, but the config is preserved
    /// so operators can toggle without losing it. Defaults to
    /// `true`.
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// STT provider selection + connection settings.
    pub provider: SttProvider,
    /// STT sampling/retry parameters.
    #[serde(default)]
    pub agent: SttConfig,
}

fn default_true() -> bool {
    true
}

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

    /// Transcribe the audio reachable via `handle` into `doc`. The
    /// handle stays as audio — downstream text detection runs
    /// through the same orchestrator tree walk.
    ///
    /// `diarization` is currently advisory — diarization is not yet
    /// implemented; a warning is logged when requested. See #239.
    pub async fn run(
        &self,
        doc: &mut nvisy_ontology::document::Document<Audio>,
        handle: &crate::core::SharedHandle,
        metadata: &nvisy_core::content::ContentMetadata,
        diarization: bool,
    ) -> Result<()> {
        // Stamp the real provenance over the importer's placeholder
        // ahead of any early returns — even an empty transcript
        // should reflect the model that ran.
        let provenance = self.stt.provenance();
        doc.meta.extraction = if diarization {
            AudioExtraction::Diarization(provenance)
        } else {
            AudioExtraction::Transcription(provenance)
        };

        let audio_data = {
            let handle = handle.lock().await;
            let DocumentHandle::Audio(ref handler) = *handle else {
                return Ok(());
            };
            handler.encode()?
        };

        if diarization {
            tracing::warn!(target: TARGET, "diarization not yet supported, skipping");
        }

        tracing::debug!(target: TARGET, "transcribing audio");
        let filename = metadata
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
        doc.blocks.push(Block::new(AudioBlock::Speech {
            time_span,
            text: stt_result.text.clone(),
            speaker_id: None,
        }));

        tracing::debug!(target: TARGET, "audio transcript captured");
        Ok(())
    }
}
