//! [`LlmRecognizer`]: adapts an [`NerPipeline`] to the
//! [`Recognizer`] trait.
//!
//! All NER-specific orchestration — detect, verify, coreference
//! merge — lives on [`NerPipeline`] in nvisy-rig. This recognizer
//! is a thin adapter: translate [`DetectionContext`] into a rig
//! [`DetectionConfig`], call [`NerPipeline::run`], and forward
//! `reset()` to [`Pipeline::reset`].
//!
//! [`DetectionContext`]: crate::DetectionContext
//! [`NerPipeline`]: nvisy_rig::pipeline::NerPipeline
//! [`NerPipeline::run`]: nvisy_rig::pipeline::NerPipeline::run
//! [`Pipeline::reset`]: nvisy_rig::pipeline::Pipeline::reset

use async_trait::async_trait;
use nvisy_ontology::entity::Entities;
use nvisy_rig::agent::DetectionConfig;
use nvisy_rig::pipeline::{NerPipeline, Pipeline};

use crate::error::{Error, Result};
use crate::{DetectionContext, Recognizer};

/// LLM-backed entity recognizer.
///
/// Wraps a pre-built [`NerPipeline`] and exposes it via the
/// [`Recognizer`] trait. Per-call detection hints
/// (`ctx.entities`, `ctx.score_threshold`) translate into the rig
/// [`DetectionConfig`] passed to [`NerPipeline::run`].
///
/// [`NerPipeline`]: nvisy_rig::pipeline::NerPipeline
/// [`NerPipeline::run`]: nvisy_rig::pipeline::NerPipeline::run
pub struct LlmRecognizer {
    pipeline: NerPipeline,
}

impl LlmRecognizer {
    /// Construct from a pre-built [`NerPipeline`].
    ///
    /// [`NerPipeline`]: nvisy_rig::pipeline::NerPipeline
    pub fn new(pipeline: NerPipeline) -> Self {
        Self { pipeline }
    }

    /// Build the rig per-call [`DetectionConfig`] from the per-call
    /// [`DetectionContext`].
    ///
    /// [`DetectionContext`]: crate::DetectionContext
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
        self.pipeline
            .run(ctx.text, &config)
            .await
            .map_err(|e| Error::Recognizer {
                name: "llm".into(),
                cause: e.to_string(),
            })
    }

    /// Clears coreference state at document boundaries by
    /// delegating to [`Pipeline::reset`].
    ///
    /// [`Pipeline::reset`]: nvisy_rig::pipeline::Pipeline::reset
    async fn reset(&self) {
        self.pipeline.reset().await;
    }
}
