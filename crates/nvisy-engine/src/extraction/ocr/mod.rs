//! [`OcrExtractor`]: pure OCR over image spans.
//!
//! Built once at engine startup from [`OcrExtractorConfig`] and
//! shared across every run via [`Extractors`]. CV verification of
//! detected entities lives in the sibling [`super::vlm`] module.
//!
//! [`Extractors`]: super::Extractors

mod params;

use nvisy_codec::Span;
use nvisy_codec::handler::ImageData;
use nvisy_core::http::{HttpClient, HttpConfig};
use nvisy_core::{Error, Result};
use nvisy_ocr::{ImageFormat, ImageInput, ImageOutput, OcrEngine, RunParams};
use nvisy_ontology::entity::ImageLocation;

pub use self::params::OcrExtractorConfig;
use crate::envelope::{Document, DocumentEnvelope};

const TARGET: &str = "nvisy_engine::extraction::ocr";

/// Pre-built OCR extractor: provider engine + run params.
pub struct OcrExtractor {
    engine: OcrEngine,
    params: RunParams,
}

impl OcrExtractor {
    /// Build from an [`OcrExtractorConfig`] bundle.
    ///
    /// # Errors
    ///
    /// Returns an error if the HTTP client cannot be constructed.
    pub fn from_config(cfg: OcrExtractorConfig) -> Result<Self> {
        let http_client = HttpClient::new(&HttpConfig::default())
            .map_err(|e| Error::runtime(e.to_string(), "ocr-http-client", false))?;
        let engine = cfg.provider.into_engine_with_client(http_client);
        Ok(Self {
            engine,
            params: cfg.policy,
        })
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
                Ok(ImageInput::with_source(
                    span.source,
                    png_bytes,
                    ImageFormat::Png,
                ))
            })
            .collect::<Result<Vec<_>>>()?;
        self.engine.run_batch(&inputs, &self.params).await
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
