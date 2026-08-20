//! Per-enricher compile helpers: language, OCR, STT.
//!
//! Symmetric with [`super::recognizers`]. Enrichers run before
//! recognition and stamp side-channel data (language hint,
//! OCR'd text layout, audio transcript segments) onto the
//! per-request context so downstream recognizers pick it up
//! transparently.
//!
//! Each enricher is at-most-one per analyzer.

#[cfg(any(feature = "internal_image", feature = "internal_audio"))]
use elide::Result;
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
use elide::modality::TextRecognizable;
#[cfg(feature = "internal_audio")]
use elide::modality::audio::Audio;
#[cfg(feature = "internal_image")]
use elide::modality::image::Image;
#[cfg(feature = "internal_image")]
use elide_bento::ocr::BentoOcr;
#[cfg(feature = "internal_audio")]
use elide_bento::stt::BentoStt;

#[cfg(feature = "internal_image")]
use crate::provider::ocr::{OcrBackend, OcrEnricherConfig};
#[cfg(feature = "internal_audio")]
use crate::provider::stt::{SttBackend, SttEnricherConfig};

/// Attach the lingua language-detection enricher.
///
/// Writes the detected language into the per-request recognizer
/// context, so pattern/NER/LLM downstream see what it wrote. The
/// detector considers every language lingua was compiled with.
///
/// Generic over [`TextRecognizable`], the modalities whose payload
/// is recognizable text: [`Text`] reads its body directly, and
/// tabular reads each cell. A caller-asserted language on the
/// request scope wins; the enricher skips detection entirely when
/// one is present.
///
/// [`TextRecognizable`]: elide::modality::TextRecognizable
/// [`Text`]: elide::modality::text::Text
pub(super) fn attach_language<M: TextRecognizable>(analyzer: Analyzer<M>) -> Analyzer<M> {
    analyzer.with_enricher(LinguaEnricher::unrestricted())
}

/// Attach an [`OcrEnricher`] for the image modality.
///
/// The deployment's `Bento` backend wraps elide-bento's
/// `BentoOcr` client.
#[cfg(feature = "internal_image")]
pub(super) fn attach_ocr(
    analyzer: Analyzer<Image>,
    config: &OcrEnricherConfig,
) -> Result<Analyzer<Image>> {
    // The deployment's name for the enricher becomes its usage id,
    // so a usage report names the enricher a caller configured
    // rather than a fixed crate string.
    let builder = OcrEnricher::builder().with_name(config.name.clone());
    let builder = match &config.backend {
        OcrBackend::Bento { base_url, model } => {
            builder.with_backend(BentoOcr::new(base_url.clone(), model.clone())?)
        }
        #[cfg(feature = "test-utils")]
        OcrBackend::Mock => builder.with_backend(MockOcrBackend),
    };
    Ok(analyzer.with_enricher(builder.build()?))
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
    config: &SttEnricherConfig,
) -> Result<Analyzer<Audio>> {
    // Same as OCR: the configured name is the usage id.
    let builder = SttEnricher::builder().with_name(config.name.clone());
    let builder = match &config.backend {
        SttBackend::Bento { base_url, model } => {
            builder.with_backend(BentoStt::new(base_url.clone(), model.clone())?)
        }
        #[cfg(feature = "test-utils")]
        SttBackend::Mock => builder.with_backend(MockSttBackend),
    };
    Ok(analyzer.with_enricher(builder.build()?))
}
