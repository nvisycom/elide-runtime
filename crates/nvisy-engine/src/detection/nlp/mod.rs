//! [`NlpRecognizer`]: NER over [`NlpEngine`].
//!
//! Wraps the NLP engine so every detection call goes through its
//! orchestration: language detection (asserted-bypass-able), NER
//! backend dispatch, and the tokens/keywords side effects (currently
//! discarded — recognition returns only entities).
//!
//! Post-filtering (entity-kind allowlist, score threshold) is
//! applied centrally at the detection layer, not inside this
//! recognizer.
//!
//! Today only [`NoopBackend`] is available — in-process model
//! backends were removed pending ecosystem stability (see
//! `nvisycom/runtime#192`, `#193`). Inference is being externalized
//! to a separate service in a follow-up PR; that path will plug an
//! HTTP backend into [`NlpEngineBuilder`] instead.
//!
//! Construct via [`from_config`] for the default no-op pipeline, or
//! [`from_engine`] to inject a pre-built engine with a custom
//! backend (tests, future HTTP backend, anything that implements
//! [`NerBackend`]).
//!
//! [`NlpEngine`]: nvisy_nlp::NlpEngine
//! [`NlpEngineBuilder`]: nvisy_nlp::NlpEngineBuilder
//! [`NoopBackend`]: nvisy_nlp::ner::NoopBackend
//! [`NerBackend`]: nvisy_nlp::ner::NerBackend
//! [`from_config`]: NlpRecognizer::from_config
//! [`from_engine`]: NlpRecognizer::from_engine

mod params;

use async_trait::async_trait;
use nvisy_core::Result;
use nvisy_nlp::language::LinguaLanguagePolicy;
use nvisy_nlp::ner::NoopBackend;
use nvisy_nlp::{NlpContext, NlpEngine};
use nvisy_ontology::entity::Entities;

pub use self::params::NlpDetection;
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
    /// Constructs an [`NlpEngine`] with [`NoopBackend`] — the only
    /// in-process backend that ships today. An externalized HTTP
    /// backend lands in a follow-up PR.
    ///
    /// # Errors
    ///
    /// Returns an error if the underlying NLP engine cannot be
    /// constructed.
    ///
    /// [`NlpEngine`]: nvisy_nlp::NlpEngine
    /// [`NoopBackend`]: nvisy_nlp::ner::NoopBackend
    pub async fn from_config(_cfg: &NlpDetection) -> Result<Self> {
        let engine = NlpEngine::builder()
            .with_ner_backend(NoopBackend)
            .with_language_policy(LinguaLanguagePolicy)
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
