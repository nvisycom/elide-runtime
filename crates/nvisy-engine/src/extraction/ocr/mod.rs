//! [`OcrExtractor`]: pure OCR over image spans.
//!
//! Built once at engine startup from [`OcrExtractorConfig`] and
//! shared across every run via [`Extractors`]. CV verification of
//! detected entities lives in the sibling [`vlm`] module.
//!
//! [`Extractors`]: super::Extractors
//! [`vlm`]: super::vlm

mod params;

use nvisy_codec::Span;
use nvisy_codec::handler::ImageData;
use nvisy_core::Result;
use nvisy_ocr::{ImageFormat, ImageInput, ImageOutput, OcrEngine, OcrParams};
use nvisy_ontology::entity::ImageLocation;

pub use self::params::OcrExtractorConfig;
use crate::envelope::{Document, DocumentEnvelope};

const TARGET: &str = "nvisy_engine::extraction::ocr";

/// Pre-built OCR extractor wrapping a configured [`OcrEngine`].
pub struct OcrExtractor {
    engine: OcrEngine,
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
        let engine = cfg.backend.into_engine()?;
        Ok(Self { engine })
    }

    /// Run OCR over the envelope's image spans, recording the
    /// per-page output on the image artifacts.
    pub async fn run(&self, envelope: &mut DocumentEnvelope) -> Result<()> {
        let spans = Self::collect_spans(&envelope.document).await;
        if spans.is_empty() {
            return Ok(());
        }

        tracing::debug!(
            target: TARGET,
            spans = spans.len(),
            "running OCR extraction",
        );

        let ocr_output = self.extract(&spans).await?;

        if let Some(image_artifacts) = envelope.document.artifacts.as_image_mut() {
            for output in &ocr_output {
                image_artifacts.ocr_pages.extend(output.pages.clone());
            }
        }

        Ok(())
    }

    async fn extract(&self, spans: &[Span<ImageLocation, ImageData>]) -> Result<Vec<ImageOutput>> {
        if spans.is_empty() {
            return Ok(Vec::new());
        }
        let inputs = spans
            .iter()
            .map(|span| {
                let png_bytes = span.data.encode_png()?;
                Ok(ImageInput::new(png_bytes, ImageFormat::Png))
            })
            .collect::<Result<Vec<_>>>()?;
        // No language hint plumbed through at this layer yet —
        // backends that need one will surface that when wired.
        self.engine.run_batch(&inputs, OcrParams::default()).await
    }

    async fn collect_spans(document: &Document) -> Vec<Span<ImageLocation, ImageData>> {
        let locations = document.collect_image_locations().await;
        let mut spans = Vec::with_capacity(locations.len());
        for located in locations {
            if let Some(data) = document.read_image(&located.location).await {
                spans.push(Span::from_located(located, data));
            }
        }
        spans
    }
}
