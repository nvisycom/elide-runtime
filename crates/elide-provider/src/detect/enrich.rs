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
use elide::backend::metrics::Metered;
use elide::detection::Analyzer;
use elide::enrichment::lingua::LinguaEnricher;
#[cfg(feature = "mocks")]
use elide::enrichment::ocr::MockBackend as MockOcrBackend;
use elide::enrichment::ocr::OcrEnricher;
#[cfg(feature = "mocks")]
use elide::enrichment::stt::MockBackend as MockSttBackend;
use elide::enrichment::stt::SttEnricher;
use elide::modality::TextRecognizable;
use elide::modality::audio::Audio;
use elide::modality::image::Image;
#[cfg(feature = "deepgram")]
use elide::{Error, ErrorKind};
use elide_bentoml::ocr::BentoOcr;
use elide_bentoml::stt::BentoStt;
#[cfg(feature = "deepgram")]
use elide_deepgram::{DeepgramStt, deepgram};

use crate::component::Component;
use crate::detect::{OcrBackend, SttBackend};
use crate::request::Resolved;

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
/// The deployment's `Bento` backend wraps elide-bentoml's
/// `BentoOcr` client.
pub(super) fn attach_ocr(
    analyzer: Analyzer<Image>,
    config: &Component<OcrBackend>,
    resolved: &Resolved<'_>,
) -> Result<Analyzer<Image>> {
    // The deployment's name for the enricher becomes its usage id,
    // so a usage report names the enricher a caller configured
    // rather than a fixed crate string.
    // The enricher is generic over its backend, so each arm builds a
    // differently-typed value and attaches it directly rather than
    // folding into one shared builder.
    Ok(match &config.backend {
        OcrBackend::Bento { base_url, model } => analyzer.with_enricher(
            OcrEnricher::new(Metered::new(
                BentoOcr::new(base_url.clone(), model.clone())?,
                resolved.sink(),
            ))
            .with_name(config.name.clone()),
        ),
        #[cfg(feature = "mocks")]
        OcrBackend::Mock => analyzer.with_enricher(
            OcrEnricher::new(Metered::new(MockOcrBackend::new(), resolved.sink()))
                .with_name(config.name.clone()),
        ),
    })
}

/// Build the Deepgram backend, going through the SDK client only
/// when the deployment overrode the base URL.
///
/// `DeepgramStt::new` builds its own client against Deepgram's
/// public endpoint, which is the common case; a regional endpoint
/// or a local stand-in needs the client built explicitly.
///
/// The `map_err` names the rejected URL: a bare conversion would
/// report only that a client could not be built, leaving the
/// operator to guess which setting was at fault.
#[cfg(feature = "deepgram")]
fn deepgram_backend(api_key: &str, base_url: Option<&str>, diarize: bool) -> Result<DeepgramStt> {
    let backend = match base_url {
        None => DeepgramStt::new(api_key)?,
        Some(base_url) => {
            let client = deepgram::Deepgram::with_base_url_and_api_key(base_url, api_key).map_err(
                |err| {
                    Error::new(
                        ErrorKind::Configuration,
                        format!("deepgram: base_url {base_url:?} did not build a client: {err}"),
                    )
                },
            )?;
            DeepgramStt::from_client(client)
        }
    };
    // Diarization is opt-in: it infers one more thing about the
    // people recorded, so a deployment asks for it rather than
    // getting it by default.
    Ok(if diarize {
        backend.with_diarization()
    } else {
        backend
    })
}

/// Attach an [`SttEnricher`] for the audio modality.
///
/// The deployment's `Bento` backend wraps elide-bentoml's
/// `BentoStt` client; `Deepgram` wraps elide-deepgram's hosted one.
///
/// [`SttEnricher`]: elide::enrichment::stt::SttEnricher
pub(super) fn attach_stt(
    analyzer: Analyzer<Audio>,
    config: &Component<SttBackend>,
    resolved: &Resolved<'_>,
) -> Result<Analyzer<Audio>> {
    // Same as OCR: the configured name is the usage id.
    // Each arm attaches its own backend type; see `attach_ocr`.
    Ok(match &config.backend {
        SttBackend::Bento { base_url, model } => analyzer.with_enricher(
            SttEnricher::new(Metered::new(
                BentoStt::new(base_url.clone(), model.clone())?,
                resolved.sink(),
            ))
            .with_name(config.name.clone()),
        ),
        #[cfg(feature = "deepgram")]
        SttBackend::Deepgram {
            api_key,
            base_url,
            diarize,
        } => analyzer.with_enricher(
            SttEnricher::new(Metered::new(
                deepgram_backend(api_key, base_url.as_deref(), *diarize)?,
                resolved.sink(),
            ))
            .with_name(config.name.clone()),
        ),
        #[cfg(feature = "mocks")]
        SttBackend::Mock => analyzer.with_enricher(
            SttEnricher::new(Metered::new(MockSttBackend::new(), resolved.sink()))
                .with_name(config.name.clone()),
        ),
    })
}
