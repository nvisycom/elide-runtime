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
//! Construct via [`from_config`] from a [`NlpDetection`] bundle —
//! the bundle's [`engine`] field selects which prebuilt [`NlpEngine`]
//! to load. [`from_engine`] is retained as an escape hatch for
//! callers that already have a constructed engine they want to
//! inject (custom backends, test fixtures, engines shared across
//! recognizers).
//!
//! [`NlpEngine`]: nvisy_nlp::NlpEngine
//! [`from_config`]: NlpRecognizer::from_config
//! [`from_engine`]: NlpRecognizer::from_engine
//! [`engine`]: NlpDetection::engine

mod params;

use async_trait::async_trait;
use nvisy_core::Result;
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
    /// The bundle's [`engine`] field picks which prebuilt
    /// [`NlpEngine`] preset to load. Async because some presets
    /// download model artifacts from HuggingFace on first use.
    ///
    /// # Errors
    ///
    /// Returns an error if the underlying NLP engine cannot be
    /// constructed (model load failure for non-default presets).
    ///
    /// [`NlpEngine`]: nvisy_nlp::NlpEngine
    /// [`engine`]: NlpDetection::engine
    pub async fn from_config(cfg: &NlpDetection) -> Result<Self> {
        let engine = cfg
            .engine
            .build()
            .await
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
            // GLiNER consumes this as `requested_kinds` (detection-
            // shaping). The post-filter pass at the detection layer
            // re-applies the allowlist on the produced entities.
            entities: ctx.entities.clone(),
            // score_threshold is *not* threaded in — applied
            // centrally at the detection layer instead.
            score_threshold: None,
            correlation_id: ctx.correlation_id,
        }
    }
}
