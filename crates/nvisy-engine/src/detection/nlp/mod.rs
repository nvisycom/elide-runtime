//! [`NlpRecognizer`]: NER over [`NlpEngine`].
//!
//! Wraps the NLP engine so every detection call goes through its
//! orchestration: language detection (asserted-bypass-able) and NER
//! backend dispatch.
//!
//! Post-filtering (entity-kind allowlist, score threshold) is
//! applied centrally at the detection layer, not inside this
//! recognizer.
//!
//! Backend selection is config-driven via [`NlpBackend`]:
//! [`NoopBackend`] for the baseline pipeline, [`BentoNerBackend`]
//! (feature `bento`) for the externalised inference service.
//!
//! Construct via [`from_config`] for the configured backend, or
//! [`from_engine`] to inject a pre-built engine with a custom
//! backend (tests, future backends, anything implementing
//! [`NerBackend`]).
//!
//! [`NlpEngine`]: nvisy_nlp::NlpEngine
//! [`NlpBackend`]: NlpBackend
//! [`NoopBackend`]: nvisy_nlp::ner::NoopBackend
//! [`BentoNerBackend`]: nvisy_nlp::ner::BentoNerBackend
//! [`NerBackend`]: nvisy_nlp::ner::NerBackend
//! [`from_config`]: NlpRecognizer::from_config
//! [`from_engine`]: NlpRecognizer::from_engine

mod params;

use async_trait::async_trait;
use nvisy_core::Result;
use nvisy_nlp::language::LinguaLanguagePolicy;
use nvisy_nlp::ner::NoopBackend;
use nvisy_nlp::{NlpContext, NlpEngine, NlpEngineBuilder};
use nvisy_ontology::entity::Entities;

pub use self::params::{NlpBackend, NlpDetection};
use crate::detection::{DetectionContext, Recognizer};

/// NER recognizer backed by [`NlpEngine`].
///
/// [`NlpEngine`]: nvisy_nlp::NlpEngine
pub struct NlpRecognizer {
    engine: NlpEngine,
}

impl NlpRecognizer {
    /// Build a recognizer from a [`NlpDetection`] config bundle.
    ///
    /// Constructs an [`NlpEngine`] with the backend the config
    /// selects ([`NlpBackend::Noop`] or [`NlpBackend::Bento`]).
    ///
    /// # Errors
    ///
    /// Returns an error if the underlying NLP engine cannot be
    /// constructed, or if the config selects a backend whose
    /// feature wasn't compiled in.
    ///
    /// [`NlpEngine`]: nvisy_nlp::NlpEngine
    /// [`NlpBackend::Noop`]: crate::detection::NlpBackend::Noop
    /// [`NlpBackend::Bento`]: crate::detection::NlpBackend::Bento
    pub async fn from_config(cfg: &NlpDetection) -> Result<Self> {
        let builder = NlpEngineBuilder::default().with_language_policy(LinguaLanguagePolicy);
        let builder = attach_backend(builder, &cfg.backend)?;
        let engine = builder
            .build()
            .map_err(|e| nvisy_core::Error::runtime(e.to_string(), "ner", false))?;
        Ok(Self::from_engine(engine))
    }

    /// Build from a pre-constructed NLP engine.
    ///
    /// Escape hatch for callers that already own an engine (custom
    /// backend, test fixture, engine shared across recognizers).
    /// Prefer [`from_config`] for ordinary use.
    ///
    /// [`from_config`]: Self::from_config
    pub fn from_engine(engine: NlpEngine) -> Self {
        Self { engine }
    }
}

/// Attach the [`NerBackend`] the config selects to the engine
/// builder.
///
/// Each variant constructs its concrete backend type and hands it
/// to [`NlpEngineBuilder::with_ner_backend`]. Bento is gated on the
/// `bento` cargo feature; selecting it without the feature compiled
/// in surfaces as a clear runtime validation error so config files
/// can stay portable across deployments.
///
/// [`NerBackend`]: nvisy_nlp::ner::NerBackend
fn attach_backend(builder: NlpEngineBuilder, backend: &NlpBackend) -> Result<NlpEngineBuilder> {
    match backend {
        NlpBackend::Noop => Ok(builder.with_ner_backend(NoopBackend)),

        #[cfg(feature = "bento")]
        NlpBackend::Bento { base_url } => {
            use nvisy_nlp::ner::{BentoNerBackend, BentoNerParams};
            let backend = BentoNerBackend::new(BentoNerParams::new(base_url.clone()))
                .map_err(|e| nvisy_core::Error::runtime(e.to_string(), "ner", false))?;
            Ok(builder.with_ner_backend(backend))
        }

        #[cfg(not(feature = "bento"))]
        NlpBackend::Bento { .. } => Err(nvisy_core::Error::validation(
            "NlpBackend::Bento requires nvisy-engine to be built with the `bento` feature",
            "ner",
        )),
    }
}

#[async_trait]
impl Recognizer for NlpRecognizer {
    type Context = NlpContext;

    #[tracing::instrument(
        skip_all,
        fields(
            text_len = text.len(),
            correlation_id = ctx.correlation_id.as_ref().map(|id| id.to_string()),
        ),
    )]
    async fn run(&self, text: &str, ctx: &NlpContext) -> Result<Entities> {
        let artifacts = self
            .engine
            .analyze(text, ctx)
            .await
            .map_err(|e| nvisy_core::Error::runtime(e.to_string(), "ner", false))?;
        Ok(artifacts.entities)
    }
}

impl From<&DetectionContext> for NlpContext {
    fn from(ctx: &DetectionContext) -> Self {
        Self {
            language: ctx.language.clone(),
            candidate_languages: ctx.candidate_languages.clone(),
            // Zero-shot backends consume this as `requested_kinds`
            // (detection-shaping). The post-filter pass at the
            // detection layer re-applies the allowlist on the
            // produced entities.
            entities: ctx.entities.clone(),
            // score_threshold is *not* threaded in — applied
            // centrally at the detection layer instead.
            score_threshold: None,
            correlation_id: ctx.correlation_id,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn from_config_noop_builds() {
        let cfg = NlpDetection {
            enabled: true,
            backend: NlpBackend::Noop,
        };
        let recognizer = match NlpRecognizer::from_config(&cfg).await {
            Ok(r) => r,
            Err(e) => panic!("from_config(Noop) failed: {e}"),
        };
        let out = recognizer
            .run("The quick brown fox", &NlpContext::default())
            .await
            .unwrap();
        assert!(out.is_empty());
    }

    #[cfg(feature = "bento")]
    #[tokio::test]
    async fn from_config_bento_with_invalid_url_errors() {
        let cfg = NlpDetection {
            enabled: true,
            backend: NlpBackend::Bento {
                base_url: "not a url".to_owned(),
            },
        };
        match NlpRecognizer::from_config(&cfg).await {
            Ok(_) => panic!("expected invalid base_url to error"),
            Err(e) => assert!(
                e.to_string().to_lowercase().contains("bento"),
                "error should mention bento: {e}",
            ),
        }
    }

    #[cfg(not(feature = "bento"))]
    #[tokio::test]
    async fn from_config_bento_without_feature_errors_clearly() {
        let cfg = NlpDetection {
            enabled: true,
            backend: NlpBackend::Bento {
                base_url: "http://localhost:3000".to_owned(),
            },
        };
        match NlpRecognizer::from_config(&cfg).await {
            Ok(_) => panic!("Bento should not build without `bento` feature"),
            Err(e) => assert!(
                e.to_string().contains("`bento` feature"),
                "error should mention the bento feature: {e}",
            ),
        }
    }
}
