//! [`NerRecognizer`]: NER over [`NerEngine`].
//!
//! Wraps the NER engine so every detection call goes through its
//! orchestration: language detection (asserted-bypass-able) and NER
//! backend dispatch.
//!
//! Post-filtering (entity-kind allowlist, score threshold) is
//! applied centrally at the detection layer, not inside this
//! recognizer.
//!
//! Backend selection is config-driven via [`NerBackend`], whose
//! [`attach_ner_backend`] helper hands the selected backend to the
//! engine builder. [`NoopBackend`] is the baseline; [`BentoBackend`]
//! (feature `bento`) is the externalised inference service.
//!
//! Construct via [`from_config`] for the configured backend, or
//! [`from_engine`] to inject a pre-built engine with a custom
//! backend (tests, future backends, anything implementing
//! [`Backend`]).
//!
//! [`Backend`]: nvisy_ner::Backend
//! [`NerEngine`]: nvisy_ner::NerEngine
//! [`NerBackend`]: nvisy_ner::NerBackend
//! [`attach_ner_backend`]: nvisy_ner::NerBackend::attach_ner_backend
//! [`NoopBackend`]: nvisy_ner::backend::NoopBackend
//! [`BentoBackend`]: nvisy_ner::backend::BentoBackend
//! [`from_config`]: NerRecognizer::from_config
//! [`from_engine`]: NerRecognizer::from_engine

mod params;

use async_trait::async_trait;
use nvisy_core::Result;
use nvisy_ner::language::LinguaLanguagePolicy;
use nvisy_ner::{NerContext, NerEngine, NerEngineBuilder};
use nvisy_ontology::entity::Entities;

pub use self::params::NerDetection;
use crate::detection::{DetectionContext, Recognizer};

/// NER recognizer backed by [`NerEngine`].
///
/// [`NerEngine`]: nvisy_ner::NerEngine
pub struct NerRecognizer {
    engine: NerEngine,
}

impl NerRecognizer {
    /// Build a recognizer from a [`NerDetection`] config bundle.
    ///
    /// Constructs an [`NerEngine`] with the backend the config
    /// selects via [`NerBackend::attach_ner_backend`].
    ///
    /// # Errors
    ///
    /// Returns an error if the underlying NER engine cannot be
    /// constructed, or if the config selects a backend whose
    /// feature wasn't compiled in.
    ///
    /// [`NerEngine`]: nvisy_ner::NerEngine
    /// [`NerBackend::attach_ner_backend`]: nvisy_ner::NerBackend::attach_ner_backend
    pub async fn from_config(cfg: &NerDetection) -> Result<Self> {
        let builder = NerEngineBuilder::default().with_language_policy(LinguaLanguagePolicy);
        let builder = cfg.backend.attach_ner_backend(builder)?;
        let engine = builder
            .build()
            .map_err(|e| nvisy_core::Error::runtime(e.to_string(), "ner", false))?;
        Ok(Self::from_engine(engine))
    }

    /// Build from a pre-constructed NER engine.
    ///
    /// Escape hatch for callers that already own an engine (custom
    /// backend, test fixture, engine shared across recognizers).
    /// Prefer [`from_config`] for ordinary use.
    ///
    /// [`from_config`]: Self::from_config
    pub fn from_engine(engine: NerEngine) -> Self {
        Self { engine }
    }
}

#[async_trait]
impl Recognizer for NerRecognizer {
    type Context = NerContext;

    #[tracing::instrument(
        skip_all,
        fields(
            text_len = text.len(),
            correlation_id = ctx.correlation_id.as_ref().map(|id| id.to_string()),
        ),
    )]
    async fn run(&self, text: &str, ctx: &NerContext) -> Result<Entities> {
        let artifacts = self.engine.analyze(text, ctx).await?;
        Ok(artifacts.entities)
    }
}

impl From<&DetectionContext> for NerContext {
    fn from(ctx: &DetectionContext) -> Self {
        Self {
            language: ctx.language.clone(),
            candidate_languages: ctx.candidate_languages.clone(),
            // Zero-shot backends consume this as `requested_kinds`
            // (detection-shaping). The post-filter pass at the
            // detection layer re-applies the allowlist on the
            // produced entities.
            entities: ctx.entities.clone(),
            correlation_id: ctx.correlation_id,
        }
    }
}

#[cfg(test)]
mod tests {
    use nvisy_ner::NerBackend;

    use super::*;

    #[tokio::test]
    async fn from_config_noop_builds() {
        let cfg = NerDetection {
            enabled: true,
            backend: NerBackend::Noop,
        };
        let recognizer = match NerRecognizer::from_config(&cfg).await {
            Ok(r) => r,
            Err(e) => panic!("from_config(Noop) failed: {e}"),
        };
        let out = recognizer
            .run("The quick brown fox", &NerContext::default())
            .await
            .unwrap();
        assert!(out.is_empty());
    }

    #[cfg(feature = "bento")]
    #[tokio::test]
    async fn from_config_bento_with_invalid_url_errors() {
        let cfg = NerDetection {
            enabled: true,
            backend: NerBackend::Bento {
                base_url: "not a url".to_owned(),
            },
        };
        match NerRecognizer::from_config(&cfg).await {
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
        let cfg = NerDetection {
            enabled: true,
            backend: NerBackend::Bento {
                base_url: "http://localhost:3000".to_owned(),
            },
        };
        match NerRecognizer::from_config(&cfg).await {
            Ok(_) => panic!("Bento should not build without `bento` feature"),
            Err(e) => assert!(
                e.to_string().contains("`bento` feature"),
                "error should mention the bento feature: {e}",
            ),
        }
    }
}
