//! Per-enricher compile helpers: language, OCR, STT.
//!
//! Symmetric with [`super::recognizers`]. Enrichers run before
//! recognition and stamp side-channel data (language hint,
//! OCR'd text layout, audio transcript segments) onto the
//! per-request context so downstream recognizers pick it up
//! transparently.
//!
//! Each enricher is at-most-one per analyzer.

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
use elide::primitive::LanguageTag;
use elide::{Error, ErrorKind, Result};
#[cfg(feature = "aws-textract")]
use elide_aws_textract::TextractOcr;
use elide_bentoml::ocr::BentoOcr;
use elide_bentoml::stt::BentoStt;
#[cfg(feature = "deepgram")]
use elide_deepgram::{DeepgramStt, deepgram};
use elide_deployment::Lingua;
#[cfg(feature = "aws-textract")]
use elide_deployment::component::AwsCredentials;
use elide_deployment::component::Component;

use super::decorate::decorate;
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
pub(super) fn attach_language<M: TextRecognizable>(
    analyzer: Analyzer<M>,
    lingua: &Lingua,
) -> Result<Analyzer<M>> {
    // Unrestricted is not the same as "no candidates": it is every
    // language the build's model knows, which is what a deployment
    // that has not narrowed detection wants.
    if lingua.is_unrestricted() {
        return Ok(analyzer.with_enricher(LinguaEnricher::unrestricted()));
    }

    // Parsed here rather than at load so the failure names the
    // deployment field, and so `elide-deployment` stays free of a
    // dependency on elide's language primitives.
    let mut candidates = Vec::with_capacity(lingua.candidates.len());
    for tag in &lingua.candidates {
        candidates.push(LanguageTag::parse(tag.clone()).map_err(|err| {
            Error::new(
                ErrorKind::Configuration,
                format!(
                    "enrichers.lingua.candidates: `{tag}` is not a valid BCP-47                      language tag: {err}"
                ),
            )
        })?);
    }
    Ok(analyzer.with_enricher(LinguaEnricher::with_candidates(candidates)))
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
            OcrEnricher::new(decorate(
                BentoOcr::new(base_url.clone(), model.clone())?,
                config.rate_limit.as_ref(),
                resolved.sink(),
            ))
            .with_name(config.name.clone()),
        ),
        #[cfg(feature = "aws-textract")]
        OcrBackend::AwsTextract {
            region,
            credentials,
        } => analyzer.with_enricher(
            OcrEnricher::new(decorate(
                textract_backend(region.as_deref(), credentials.as_ref()),
                config.rate_limit.as_ref(),
                resolved.sink(),
            ))
            .with_name(config.name.clone()),
        ),
        #[cfg(feature = "google-document-ai")]
        OcrBackend::GoogleDocumentAi { .. } => {
            // Built at startup, not here: the client builder is
            // async and this compile is not. A provider built
            // through `Provider::new` has no such client, so refuse
            // rather than skip the enricher — a document that
            // silently got no OCR reads exactly like one with no
            // text in its images.
            let client = resolved
                .clients()
                .document_ai
                .get(config.name.as_str())
                .ok_or_else(|| {
                    Error::new(
                        ErrorKind::Configuration,
                        format!(
                            "enrichers.ocr `{}` names Document AI, whose client must be \
                         built before a request arrives. Build the provider with \
                         `Provider::try_new` instead of `Provider::new`.",
                            config.name,
                        ),
                    )
                })?;
            analyzer.with_enricher(
                OcrEnricher::new(decorate(
                    client.clone(),
                    config.rate_limit.as_ref(),
                    resolved.sink(),
                ))
                .with_name(config.name.clone()),
            )
        }
        #[cfg(feature = "mocks")]
        OcrBackend::Mock => analyzer.with_enricher(
            OcrEnricher::new(decorate(
                MockOcrBackend::new(),
                config.rate_limit.as_ref(),
                resolved.sink(),
            ))
            .with_name(config.name.clone()),
        ),
    })
}

/// Build the Textract backend from its deployment configuration.
///
/// Synchronous, unlike Document AI: `Client::from_conf` takes a
/// plain `Config`, so a region and optional static credentials are
/// all this needs. With neither, the SDK resolves both from its own
/// chain — IAM role, instance metadata, profile, environment —
/// which is what a deployment on AWS infrastructure wants and why
/// both config fields are optional.
///
/// `behavior_version_latest` is set because the SDK requires a
/// behaviour version and refuses to build without one.
#[cfg(feature = "aws-textract")]
fn textract_backend(region: Option<&str>, credentials: Option<&AwsCredentials>) -> TextractOcr {
    use elide_aws_textract::textract::config::{Credentials, Region};
    use elide_aws_textract::textract::{Client, Config};

    let mut config = Config::builder().behavior_version_latest();
    if let Some(region) = region {
        config = config.region(Region::new(region.to_owned()));
    }
    if let Some(creds) = credentials {
        config = config.credentials_provider(Credentials::new(
            creds.access_key_id.clone(),
            creds.secret_access_key.clone(),
            creds.session_token.clone(),
            // No expiry: static credentials from configuration do
            // not rotate themselves, which is why an IAM role is
            // preferred and this path is the fallback.
            None,
            "elide-deployment",
        ));
    }
    TextractOcr::new(Client::from_conf(config.build()))
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
