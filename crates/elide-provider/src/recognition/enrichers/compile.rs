//! Per-enricher compile helpers: language, OCR, STT.
//!
//! Symmetric with [`super::recognizers`]. Enrichers run before
//! recognition and stamp side-channel data (language hint,
//! OCR'd text layout, audio transcript segments) onto the
//! per-request context so downstream recognizers pick it up
//! transparently.
//!
//! Each enricher is at-most-one per analyzer.

use elide::Result;
use elide::detection::Analyzer;
use elide::enrichment::lingua::LinguaEnricher;
#[cfg(feature = "test-utils")]
use elide::enrichment::ocr::MockBackend as MockOcrBackend;
use elide::enrichment::ocr::OcrEnricher;
#[cfg(feature = "test-utils")]
use elide::enrichment::stt::MockBackend as MockSttBackend;
use elide::enrichment::stt::SttEnricher;
use elide::modality::TextRecognizable;
use elide::modality::audio::Audio;
use elide::modality::image::Image;
use elide_bento::ocr::BentoOcr;
use elide_bento::stt::BentoStt;

use super::super::Component;
use crate::recognition::{OcrBackend, SttBackend};

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
pub(in crate::recognition) fn attach_language<M: TextRecognizable>(
    analyzer: Analyzer<M>,
) -> Analyzer<M> {
    analyzer.with_enricher(LinguaEnricher::unrestricted())
}

/// Attach an [`OcrEnricher`] for the image modality.
///
/// The deployment's `Bento` backend wraps elide-bento's
/// `BentoOcr` client.
pub(in crate::recognition) fn attach_ocr(
    analyzer: Analyzer<Image>,
    config: &Component<OcrBackend>,
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
pub(in crate::recognition) fn attach_stt(
    analyzer: Analyzer<Audio>,
    config: &Component<SttBackend>,
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
