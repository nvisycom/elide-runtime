//! Attach the OCR enricher to an image-modality [`Analyzer`].
//!
//! Image-modality only. The deployment's `Bento` backend wraps
//! elide-bento's `BentoOcr` client; unknown non-exhaustive
//! variants surface as Validation instead of silently skipping
//! OCR.

use elide::detection::Analyzer;
#[cfg(feature = "test-utils")]
use elide::enrichment::ocr::MockBackend as MockOcrBackend;
use elide::enrichment::ocr::OcrEnricher;
use elide_bento::ocr::BentoOcr;
use elide_core::modality::image::Image;
use elide_core::{Error, ErrorKind};
use nvisy_schema::plan::{OcrBackendParams, OcrEnricherParams};

/// Attach an [`OcrEnricher`] for the image modality.
pub(in crate::analyzer) fn attach(
    analyzer: Analyzer<Image>,
    spec: &OcrEnricherParams,
) -> Result<Analyzer<Image>, Error> {
    let enricher = match &spec.backend {
        OcrBackendParams::Bento { base_url, model } => {
            OcrEnricher::new(BentoOcr::new(base_url.clone(), model.clone())?)
        }
        #[cfg(feature = "test-utils")]
        OcrBackendParams::Mock => OcrEnricher::new(MockOcrBackend),
        // `OcrBackendParams` is `#[non_exhaustive]`. Unknown
        // variants surface as Validation instead of silently
        // skipping OCR.
        _ => {
            return Err(Error::new(
                ErrorKind::CapabilityUnavailable,
                "OCR enricher uses a backend kind this engine binary doesn't \
                 understand; upgrade the engine or downgrade the config",
            ));
        }
    };
    Ok(analyzer.with_enricher(enricher))
}
