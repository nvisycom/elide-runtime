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
#[cfg(feature = "gladia")]
use elide::{Error, ErrorKind};
use elide_bentoml::ocr::BentoOcr;
use elide_bentoml::stt::BentoStt;
#[cfg(feature = "gladia")]
use elide_gladia::{GladiaStt, gladia};

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
/// The deployment's `Bento` backend wraps elide-bentoml's
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
        OcrBackend::Mock => builder.with_backend(MockOcrBackend::new()),
    };
    Ok(analyzer.with_enricher(builder.build()?))
}

/// Build the Gladia backend, going through the SDK client only
/// when the deployment overrode the base URL.
///
/// `GladiaStt::new` builds its own client against Gladia's public
/// endpoint, which is the common case; a regional endpoint or a
/// local stand-in needs the client built explicitly.
///
/// The `map_err` mirrors what `GladiaStt::new` does internally.
/// elide-gladia keeps its `GladiaError` conversion crate-private
/// and the SDK's own error has no `From` into elide's, so there is
/// nothing to delegate to from here. Naming the rejected URL is
/// worth the closure regardless: a bare conversion would report
/// only that a client could not be built.
#[cfg(feature = "gladia")]
fn gladia_backend(api_key: &str, base_url: Option<&str>) -> Result<GladiaStt> {
    let Some(base_url) = base_url else {
        return GladiaStt::new(api_key);
    };
    let client = gladia::Client::builder()
        .with_api_key(api_key)
        .with_base_url(base_url)
        .build()
        .map_err(|err| {
            Error::new(
                ErrorKind::Configuration,
                format!("gladia: base_url {base_url:?} did not build a client: {err}"),
            )
        })?;
    GladiaStt::from_client(client)
}

/// Attach an [`SttEnricher`] for the audio modality.
///
/// The deployment's `Bento` backend wraps elide-bentoml's
/// `BentoStt` client; `Gladia` wraps elide-gladia's hosted one.
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
        #[cfg(feature = "gladia")]
        SttBackend::Gladia { api_key, base_url } => {
            builder.with_backend(gladia_backend(api_key, base_url.as_deref())?)
        }
        #[cfg(feature = "test-utils")]
        SttBackend::Mock => builder.with_backend(MockSttBackend::new()),
    };
    Ok(analyzer.with_enricher(builder.build()?))
}
