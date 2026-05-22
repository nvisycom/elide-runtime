//! [`NerRecognizer`]: NER over `nvisy_nlp::Engine`.
//!
//! Wraps the NLP engine so every detection call goes through its
//! orchestration: language detection (asserted-bypass-able), NER
//! backend dispatch, post-filtering by entity-kind allowlist and
//! score threshold, and the tokens/keywords side effects (currently
//! discarded — recognition returns only entities).
//!
//! Construct via [`from_config`] from a [`NerDetection`] bundle —
//! the bundle's [`engine`] field selects which prebuilt
//! [`nvisy_nlp::Engine`] to load. [`from_engine`] is retained as
//! an escape hatch for callers that already have an `Arc<NlpEngine>`
//! they want to inject (custom backends, test fixtures, shared
//! engines across recognizers).
//!
//! [`from_config`]: NerRecognizer::from_config
//! [`from_engine`]: NerRecognizer::from_engine
//! [`engine`]: NerDetection::engine

mod params;

use std::sync::Arc;

use async_trait::async_trait;
use nvisy_codec::handler::TextData;
use nvisy_core::Result;
use nvisy_core::detection::Recognizer;
use nvisy_nlp::{Context as NlpContext, Engine as NlpEngine};
use nvisy_ontology::entity::{Entities, EntityKind};
use nvisy_ontology::primitive::LanguageTag;
use uuid::Uuid;

pub use self::params::NerDetection;
use crate::detection::DetectionContext;

/// Per-call input to [`NerRecognizer::run`].
#[derive(Debug, Clone)]
pub struct NerContext {
    /// The text to analyze.
    pub text: TextData,
    /// Caller-asserted language. When `Some`, NER skips per-call
    /// language detection.
    pub language: Option<LanguageTag>,
    /// Restrict language detection to this subset. Ignored when
    /// `language` is `Some`.
    pub candidate_languages: Option<Vec<LanguageTag>>,
    /// Entity-kind allowlist. Empty = all kinds permitted.
    pub entities: Option<Vec<EntityKind>>,
    /// Minimum confidence threshold in `[0.0, 1.0]`.
    pub score_threshold: Option<f64>,
    /// Correlation UUID propagated through the tracing span.
    pub correlation_id: Option<Uuid>,
}

impl From<&DetectionContext> for NerContext {
    fn from(ctx: &DetectionContext) -> Self {
        Self {
            text: ctx.text.clone(),
            language: ctx.language.clone(),
            candidate_languages: ctx.candidate_languages.clone(),
            entities: ctx.entities.clone(),
            score_threshold: ctx.score_threshold,
            correlation_id: ctx.correlation_id,
        }
    }
}

/// NER recognizer backed by [`nvisy_nlp::Engine`].
pub struct NerRecognizer {
    engine: Arc<NlpEngine>,
}

impl NerRecognizer {
    /// Build a recognizer from a [`NerDetection`] config bundle.
    ///
    /// The bundle's [`engine`] field picks which prebuilt
    /// [`nvisy_nlp::Engine`] preset to load.
    ///
    /// # Errors
    ///
    /// Returns an error if the underlying NLP engine cannot be
    /// constructed (model load failure for non-default presets).
    ///
    /// [`engine`]: NerDetection::engine
    pub fn from_config(cfg: &NerDetection) -> Result<Self> {
        let engine = cfg
            .engine
            .build()
            .map_err(|e| nvisy_core::Error::runtime(e.to_string(), "ner", false))?;
        Ok(Self::from_engine(engine))
    }

    /// Build from a pre-constructed shared NLP engine.
    ///
    /// Escape hatch for callers that already own an engine (custom
    /// backend, test fixture, engine shared across recognizers).
    /// Prefer [`from_config`] for ordinary use.
    ///
    /// [`from_config`]: Self::from_config
    pub fn from_engine(engine: Arc<NlpEngine>) -> Self {
        Self { engine }
    }
}

#[async_trait]
impl Recognizer for NerRecognizer {
    type Context = NerContext;

    #[tracing::instrument(
        skip_all,
        fields(
            text_len = ctx.text.len(),
            correlation_id = ctx.correlation_id.as_ref().map(|id| id.to_string()),
        ),
    )]
    async fn run(&self, ctx: &NerContext) -> Result<Entities> {
        let mut nlp_ctx = NlpContext::new(&ctx.text);
        if let Some(language) = ctx.language.clone() {
            nlp_ctx.language = Some(language);
        }
        if let Some(candidates) = ctx.candidate_languages.clone() {
            nlp_ctx.candidate_languages = Some(candidates);
        }
        if let Some(entities) = ctx.entities.clone() {
            nlp_ctx.entities = Some(entities);
        }
        if let Some(threshold) = ctx.score_threshold {
            nlp_ctx.score_threshold = Some(threshold);
        }
        if let Some(id) = ctx.correlation_id {
            nlp_ctx.correlation_id = Some(id);
        }

        let artifacts = self
            .engine
            .analyze(nlp_ctx)
            .await
            .map_err(|e| nvisy_core::Error::runtime(e.to_string(), "ner", false))?;
        Ok(artifacts.entities)
    }
}
