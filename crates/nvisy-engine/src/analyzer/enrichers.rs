//! Per-enricher compile helpers: language, OCR, STT.
//!
//! Symmetric with [`super::recognizers`]. Enrichers run before
//! recognition and stamp side-channel data (language hint,
//! OCR'd text layout, audio transcript segments) onto the
//! per-request context so downstream recognizers pick it up
//! transparently.
//!
//! Each enricher is at-most-one per analyzer.

use elide::detection::Analyzer;
use elide::enrichment::lingua::LinguaEnricher;
#[cfg(feature = "test-utils")]
use elide::enrichment::ocr::MockBackend as MockOcrBackend;
#[cfg(feature = "internal_image")]
use elide::enrichment::ocr::OcrEnricher;
#[cfg(feature = "test-utils")]
use elide::enrichment::stt::MockBackend as MockSttBackend;
#[cfg(feature = "internal_audio")]
use elide::enrichment::stt::SttEnricher;
#[cfg(feature = "internal_image")]
use elide_bento::ocr::BentoOcr;
#[cfg(feature = "internal_audio")]
use elide_bento::stt::BentoStt;
#[cfg(any(feature = "internal_image", feature = "internal_audio"))]
use elide_core::Result;
#[cfg(feature = "internal_audio")]
use elide_core::modality::audio::Audio;
#[cfg(feature = "internal_image")]
use elide_core::modality::image::Image;
use elide_core::modality::text::Text;

#[cfg(feature = "internal_image")]
use crate::provider::ocr::OcrBackend;
#[cfg(feature = "internal_audio")]
use crate::provider::stt::SttBackend;

/// Attach the lingua language-detection enricher.
///
/// Text-modality only — writes the detected language into the
/// per-request recognizer context, so pattern/NER/LLM downstream
/// see what it wrote. The detector considers every language
/// lingua was compiled with.
pub(super) fn attach_language(analyzer: Analyzer<Text>) -> Analyzer<Text> {
    analyzer.with_enricher(LinguaEnricher::unrestricted())
}

/// Attach an [`OcrEnricher`] for the image modality.
///
/// The deployment's `Bento` backend wraps elide-bento's
/// `BentoOcr` client.
#[cfg(feature = "internal_image")]
pub(super) fn attach_ocr(
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

/// Attach an [`SttEnricher`] for the audio modality.
///
/// The deployment's `Bento` backend wraps elide-bento's
/// `BentoStt` client.
///
/// [`SttEnricher`]: elide::enrichment::stt::SttEnricher
#[cfg(feature = "internal_audio")]
pub(super) fn attach_stt(
    analyzer: Analyzer<Audio>,
    backend: &SttBackend,
) -> Result<Analyzer<Audio>> {
    let enricher = match backend {
        SttBackend::Bento { base_url, model } => {
            SttEnricher::new(BentoStt::new(base_url.clone(), model.clone())?)
        }
        #[cfg(feature = "test-utils")]
        SttBackend::Mock => SttEnricher::new(MockSttBackend),
    };
    Ok(analyzer.with_enricher(enricher))
}
