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
use elide::enrichment::ocr::OcrEnricher;
#[cfg(feature = "test-utils")]
use elide::enrichment::stt::MockBackend as MockSttBackend;
use elide::enrichment::stt::SttEnricher;
use elide::modality::TextRecognizable;
use elide::modality::audio::Audio;
use elide::modality::image::Image;
use elide::{Error, ErrorKind, Result};
use elide_bentoml::ocr::BentoOcr;
use elide_bentoml::stt::BentoStt;
#[cfg(feature = "gladia")]
use elide_gladia::{GladiaStt, gladia};
use url::Url;

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

/// Reject a base URL carrying userinfo, before it reaches a log.
///
/// `https://user:hunter2@host` is a password in a field the config
/// treats as public: it is `Serialize`d back out and printed by
/// `Debug`, so it would reach a config dump, a startup log, or a
/// crash report in plaintext. Credentials belong in the backend's
/// own key field, which is redacted on both paths.
///
/// Refused at compile rather than redacted at each output, so a
/// future output path cannot reintroduce the leak by forgetting.
///
/// Parsed with [`url`] rather than by hand, and deliberately the
/// same parser the backends themselves use — `bentoml` and
/// `gladia` both depend on it — so what we accept is what they
/// will resolve.
fn reject_userinfo(base_url: &str, component: &str) -> Result<()> {
    let parsed = Url::parse(base_url).map_err(|err| {
        Error::new(
            ErrorKind::Configuration,
            format!("`{component}`: base_url {base_url:?} is not a URL: {err}"),
        )
    })?;
    // A base URL the backend can resolve needs a host. Without one
    // there is no userinfo to find either: `user:hunter2@host`
    // parses as the *scheme* `user` with no host at all, so the
    // credential would slip past a userinfo-only check.
    if parsed.host_str().is_none() {
        return Err(Error::new(
            ErrorKind::Configuration,
            format!("`{component}`: base_url {base_url:?} names no host"),
        ));
    }
    if parsed.username().is_empty() && parsed.password().is_none() {
        return Ok(());
    }
    Err(Error::new(
        ErrorKind::Configuration,
        format!(
            "`{component}`: base_url carries userinfo before the host, which \
             would be written to logs and config dumps in plaintext. Put the \
             credential in the backend's own key field and give base_url the \
             bare scheme, host and port.",
        ),
    ))
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
            reject_userinfo(base_url, &config.name)?;
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
    reject_userinfo(base_url, "gladia")?;
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
            reject_userinfo(base_url, &config.name)?;
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

#[cfg(test)]
mod tests {
    use super::reject_userinfo;

    /// A credential in the URL is a password in a field the config
    /// treats as public, so it is refused rather than logged.
    #[test]
    fn a_base_url_carrying_userinfo_is_refused() {
        for carries in [
            "https://user:hunter2@eu.gladia.io",
            "https://token@bento.internal:8080",
        ] {
            let err = reject_userinfo(carries, "probe").expect_err(carries);
            assert!(
                err.to_string().contains("userinfo"),
                "the error says what is wrong: {err}",
            );
            assert!(
                !err.to_string().contains("hunter2"),
                "and does not repeat the credential back into the log: {err}",
            );
        }
    }

    /// A schemeless base URL is refused for naming no host, which
    /// is the check that catches it: `user:hunter2@host` parses
    /// happily as the *scheme* `user` with an empty username, so a
    /// userinfo-only test would wave the credential through.
    #[test]
    fn a_base_url_naming_no_host_is_refused() {
        for hostless in ["user:hunter2@host", "mailto:someone@example.com"] {
            let err = reject_userinfo(hostless, "probe").expect_err(hostless);
            assert!(
                err.to_string().contains("names no host"),
                "the error says what is missing: {err}",
            );
        }
    }

    /// The check reads the authority only, so an `@` in a path or a
    /// query is not a credential and does not trip it.
    #[test]
    fn an_at_sign_past_the_authority_is_left_alone() {
        for clean in [
            "https://api.gladia.io",
            "http://localhost:8080",
            "https://bento.internal/v1/e@mail",
            "https://bento.internal/x?to=a@b.com",
            "https://bento.internal#a@b",
        ] {
            reject_userinfo(clean, "probe").unwrap_or_else(|e| panic!("{clean} refused: {e}"));
        }
    }
}
