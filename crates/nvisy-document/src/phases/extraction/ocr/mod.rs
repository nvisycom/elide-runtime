//! [`OcrExtractor`]: pure OCR over image spans.
//!
//! Built once at engine startup from [`OcrExtractorConfig`] and
//! shared across every run via [`ExtractionEngine`].
//!
//! [`ExtractionEngine`]: super::ExtractionEngine

use futures::StreamExt;
use nvisy_codec::core::Located;
use nvisy_codec::handler::ImageData;
use nvisy_core::Result;
use nvisy_core::modality::Image;
use nvisy_ocr::core::{OcrBlockKind, OcrOutput};
use nvisy_ocr::{Context as OcrContext, ImageFormat, ImageInput};

use crate::core::SharedHandle;
use crate::document::{Block, Document, Span};
use crate::modality::{ImageBlock, ImageExtraction};
use crate::pipeline::OcrExtractorConfig;

const TARGET: &str = "nvisy_engine::extraction::image::ocr";

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

    /// Run OCR over the image regions reachable via `handle`,
    /// populating `doc`. Stamps the backend's provenance into
    /// [`ImageMetadata::extraction`] (replacing the
    /// [`ImageExtraction::Pending`] tag the importer set at document
    /// creation).
    ///
    /// Takes `(doc, handle)` rather than an envelope so the
    /// nested-document embed flow (PDF text → embedded image doc) can
    /// reuse it against a `Document<Image>` whose codec handle lives
    /// on the *outer* text envelope.
    ///
    /// [`ImageMetadata::extraction`]: crate::modality::ImageMetadata::extraction
    /// [`ImageExtraction::Pending`]: crate::modality::ImageExtraction::Pending
    pub async fn run_on_doc(&self, doc: &mut Document<Image>, handle: &SharedHandle) -> Result<()> {
        doc.meta.extraction = ImageExtraction::Ocr(self.inner.provenance());

        let inputs = Self::collect_inputs(handle).await;
        if inputs.is_empty() {
            return Ok(());
        }

        tracing::debug!(
            target: TARGET,
            regions = inputs.len(),
            "running OCR extraction",
        );

        let blocks = self.extract(&inputs).await?;
        doc.blocks.extend(blocks);
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
        let outputs = self
            .inner
            .extract_batch(&ocr_inputs, OcrContext::default())
            .await?;
        Ok(outputs.into_iter().map(output_to_block).collect())
    }
}

/// Lossless conversion from the backend-shaped [`OcrOutput`] to a
/// document-shaped [`Block<Image>`].
fn output_to_block(output: OcrOutput) -> Block<Image> {
    let kind = match output.kind {
        OcrBlockKind::Text { region, text } => ImageBlock::Text { region, text },
        OcrBlockKind::Heading { region, text } => ImageBlock::Heading { region, text },
        OcrBlockKind::Table { region, text } => ImageBlock::Table { region, text },
        // Forward-compat with new OCR backend output variants.
        _ => unreachable!("OcrBlockKind has no further variants"),
    };
    let spans: Vec<Span<Image>> = output
        .spans
        .into_iter()
        .map(|s| Span {
            text_start: s.text_start,
            text_end: s.text_end,
            source: s.source,
            confidence: Some(s.confidence),
        })
        .collect();
    Block {
        kind,
        spans,
        confidence: Some(output.confidence),
    }
}

impl OcrExtractor {
    async fn collect_inputs(handle: &SharedHandle) -> Vec<Located<Image, ImageData>> {
        let guard = handle.lock().await;
        let locations: Vec<Located<Image>> = guard.image_locations().collect().await;
        drop(guard);
        let mut out = Vec::with_capacity(locations.len());
        for located in locations {
            if let Some(data) = handle.lock().await.read_image(&located.location).await {
                out.push(located.with_data(data));
            }
        }
        out
    }
}
