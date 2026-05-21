//! [`LlmRecognizer`]: LLM-driven NER over `nvisy_rig::agent::NerAgent`.
//!
//! Wraps a constructed `NerAgent`. Per-call detection hints come
//! exclusively from [`DetectionContext`]: `entities` (kind allowlist)
//! and `score_threshold` are translated into the rig
//! `DetectionConfig` for each `recognize` call. The agent's system
//! prompt is its own per-build concern.
//!
//! The agent manages coreference state internally between
//! successive `recognize` calls; call [`reset`] at document
//! boundaries to clear it.
//!
//! [`DetectionContext`]: crate::DetectionContext
//! [`reset`]: Recognizer::reset

use async_trait::async_trait;
use nvisy_ontology::entity::Entities;
use nvisy_rig::agent::{DetectionConfig, NerAgent};

use crate::error::{Error, Result};
use crate::{DetectionContext, Recognizer};

/// LLM-backed entity recognizer.
pub struct LlmRecognizer {
    agent: NerAgent,
}

impl LlmRecognizer {
    /// Construct from a pre-built [`NerAgent`]. Per-call detection
    /// hints (entity-kind allowlist, score threshold) flow through
    /// [`DetectionContext`] at recognize time.
    ///
    /// [`DetectionContext`]: crate::DetectionContext
    pub fn new(agent: NerAgent) -> Self {
        Self { agent }
    }

    /// Build the rig per-call config from a DetectionContext.
    fn build_config(ctx: &DetectionContext<'_>) -> DetectionConfig {
        DetectionConfig {
            entity_kinds: ctx.entities.clone().unwrap_or_default(),
            confidence_threshold: ctx.score_threshold,
            system_prompt: None,
        }
    }
}

#[async_trait]
impl Recognizer for LlmRecognizer {
    #[tracing::instrument(
        skip_all,
        fields(
            text_len = ctx.text.len(),
            correlation_id = ctx.correlation_id.as_ref().map(|id| id.to_string()),
        ),
    )]
    async fn recognize(&self, ctx: &DetectionContext<'_>) -> Result<Entities> {
        let config = Self::build_config(ctx);
        let entities: Entities = self
            .agent
            .detect_entities(ctx.text, &config)
            .await
            .map_err(|e| Error::Recognizer {
                name: "llm".into(),
                cause: e.to_string(),
            })?
            .into();
        Ok(entities)
    }

    /// Clears coreference state on the wrapped `NerAgent`.
    async fn reset(&self) {
        self.agent.reset().await;
    }
}
