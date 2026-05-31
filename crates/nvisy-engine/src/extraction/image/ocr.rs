//! [`OcrExtractor`]: pure OCR over image spans.
//!
//! Built once at engine startup from [`OcrExtractorConfig`] and
//! shared across every run via [`ExtractionEngine`].
//!
//! [`ExtractionEngine`]: super::super::ExtractionEngine

use nvisy_codec::core::Located;
use nvisy_codec::handler::ImageData;
use nvisy_core::Result;
use nvisy_ocr::{Context as OcrContext, ImageFormat, ImageInput, OcrBackend};
use nvisy_ontology::document::Block;
use nvisy_ontology::modality::{Image, ImageExtraction};
use serde::{Deserialize, Serialize};

use crate::envelope::DocumentEnvelope;

const TARGET: &str = "nvisy_engine::extraction::image::ocr";

/// `[extractor.ocr]` config bundle.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OcrExtractorConfig {
    /// Enable this extractor. When `false`, the extractor is
    /// neither built nor dispatched, but the config is preserved
    /// so operators can toggle without losing it. Defaults to
    /// `true`.
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// OCR backend selection + connection settings.
    #[serde(default)]
    pub backend: OcrBackend,
}

fn default_true() -> bool {
    true
}

/// Pre-built OCR extractor wrapping a configured
/// [`nvisy_ocr::Extractor`].
pub struct OcrExtractor {
    inner: nvisy_ocr::Extractor,
}

impl OcrExtractor {
    /// Build from an [`OcrExtractorConfig`] bundle.
    ///
    /// # Errors
    ///
    /// Returns an error if the selected backend cannot be
    /// constructed, or if the config selects a backend whose
    /// feature wasn't compiled in.
    pub fn from_config(cfg: OcrExtractorConfig) -> Result<Self> {
        let inner = cfg.backend.into_extractor()?;
        Ok(Self { inner })
    }

    /// Run OCR over the envelope's image regions, appending the
    /// recognised blocks to [`DocumentEnvelope::document`] and
    /// stamping the backend's provenance into
    /// [`ImageMetadata::extraction`] (replacing the
    /// [`ImageExtraction::Pending`] tag the importer set at envelope
    /// creation).
    ///
    /// [`ImageMetadata::extraction`]: nvisy_ontology::modality::ImageMetadata::extraction
    /// [`ImageExtraction::Pending`]: nvisy_ontology::modality::ImageExtraction::Pending
    pub async fn run(&self, envelope: &mut DocumentEnvelope<Image>) -> Result<()> {
        envelope.document.meta.extraction = ImageExtraction::Ocr(self.inner.provenance());

        let inputs = Self::collect_inputs(envelope).await;
        if inputs.is_empty() {
            return Ok(());
        }

        tracing::debug!(
            target: TARGET,
            regions = inputs.len(),
            "running OCR extraction",
        );

        let blocks = self.extract(&inputs).await?;
        envelope.document.blocks.extend(blocks);
        Ok(())
    }

    async fn extract(&self, inputs: &[Located<Image, ImageData>]) -> Result<Vec<Block<Image>>> {
        let ocr_inputs = inputs
            .iter()
            .map(|item| {
                let png_bytes = item.data.encode_png()?;
                Ok(ImageInput::new(png_bytes, ImageFormat::Png))
            })
            .collect::<Result<Vec<_>>>()?;
        // No language hint plumbed through at this layer yet —
        // backends that need one will surface that when wired.
        self.inner
            .extract_batch(&ocr_inputs, OcrContext::default())
            .await
    }

    async fn collect_inputs(envelope: &DocumentEnvelope<Image>) -> Vec<Located<Image, ImageData>> {
        let locations = envelope.collect_image_locations().await;
        let mut out = Vec::with_capacity(locations.len());
        for located in locations {
            if let Some(data) = envelope.read_image(&located.location).await {
                out.push(located.with_data(data));
            }
        }
        out
    }
}
