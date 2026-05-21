//! [`NerRecognizer`]: NER over `nvisy_nlp::Engine`.
//!
//! Wraps the NLP engine so every detection call goes through its
//! orchestration: language detection (asserted-bypass-able), NER
//! backend dispatch, post-filtering by entity-kind allowlist and
//! score threshold, and the tokens/keywords side effects (currently
//! discarded — recognition returns only entities).
//!
//! Construct with [`new`] from a shared `Arc<nvisy_nlp::Engine>`.
//! The engine is constructed once at process startup and shared
//! across runs.
//!
//! [`new`]: NerRecognizer::new

use std::sync::Arc;

use async_trait::async_trait;
use nvisy_nlp::{Context as NlpContext, Engine as NlpEngine};
use nvisy_ontology::entity::Entities;

use crate::error::{Error, Result};
use crate::{DetectionContext, Recognizer};

/// NER recognizer backed by [`nvisy_nlp::Engine`].
pub struct NerRecognizer {
    engine: Arc<NlpEngine>,
}

impl NerRecognizer {
    /// Construct from a shared NLP engine.
    pub fn new(engine: Arc<NlpEngine>) -> Self {
        Self { engine }
    }
}

#[async_trait]
impl Recognizer for NerRecognizer {
    #[tracing::instrument(
        skip_all,
        fields(
            text_len = ctx.text.len(),
            correlation_id = ctx.correlation_id.as_ref().map(|id| id.to_string()),
        ),
    )]
    async fn run(&self, ctx: &DetectionContext<'_>) -> Result<Entities> {
        let mut nlp_ctx = NlpContext::new(ctx.text);
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
            .map_err(|e| Error::Recognizer {
                name: "ner".into(),
                cause: e.to_string(),
            })?;
        Ok(artifacts.entities)
    }
}
