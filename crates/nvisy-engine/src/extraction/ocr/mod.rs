//! [`OcrExtractor`]: pure OCR over image spans.
//!
//! Built once at engine startup from [`OcrExtractorConfig`] and
//! shared across every run via [`Extractors`]. CV verification of
//! detected entities lives in the sibling [`vlm`] module.
//!
//! [`Extractors`]: super::Extractors
//! [`vlm`]: super::vlm

mod params;

use nvisy_codec::core::Located;
use nvisy_codec::handler::ImageData;
use nvisy_core::Result;
use nvisy_ocr::{Context as OcrContext, ImageFormat, ImageInput};
use nvisy_ontology::document::Block;
use nvisy_ontology::modality::{Image, ImageExtraction};

pub use self::params::OcrExtractorConfig;
use crate::envelope::DocumentEnvelope;

const TARGET: &str = "nvisy_engine::extraction::ocr";

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
