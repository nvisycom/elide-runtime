//! Attach the OCR enricher to an image-modality [`Analyzer`].
//!
//! Image-modality only. The deployment's `Bento` backend wraps
//! elide-bento's `BentoOcr` client.

use elide::detection::Analyzer;
#[cfg(feature = "test-utils")]
use elide::enrichment::ocr::MockBackend as MockOcrBackend;
use elide::enrichment::ocr::OcrEnricher;
use elide_bento::ocr::BentoOcr;
use elide_core::Result;
use elide_core::modality::image::Image;

use crate::provider::ocr::OcrBackend;

/// Attach an [`OcrEnricher`] for the image modality.
pub(in crate::analyzer) fn attach(
    analyzer: Analyzer<Image>,
    backend: &OcrBackend,
) -> Result<Analyzer<Image>> {
    let enricher = match backend {
        OcrBackend::Bento { base_url, model } => {
            OcrEnricher::new(BentoOcr::new(base_url.clone(), model.clone())?)
        }
        #[cfg(feature = "test-utils")]
        OcrBackend::Mock => OcrEnricher::new(MockOcrBackend),
    };
    Ok(analyzer.with_enricher(enricher))
}
