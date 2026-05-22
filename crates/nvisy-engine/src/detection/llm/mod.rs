//! [`LlmRecognizer`]: adapts an [`NerPipeline`] to the
//! [`Recognizer`] trait.
//!
//! All NER-specific orchestration — detect, verify, coreference
//! merge — lives on [`NerPipeline`] in nvisy-agent. This recognizer
//! is a thin adapter: translate [`LlmContext`] into a rig
//! [`DetectionConfig`], call [`NerPipeline::run`], and forward
//! `reset()` to [`Pipeline::reset`].
//!
//! # Construction
//!
//! [`new`] consumes a single [`LlmDetection`] bundle and builds
//! everything internally via [`NerPipeline::new`]. The presence of
//! [`LlmDetection::verifier`] decides whether the pipeline gets a
//! localization-only verifier or the two-pass refinement verifier.
//!
//! [`from_pipeline`] is retained as an escape hatch for callers
//! that already own a [`NerPipeline`].
//!
//! [`NerPipeline`]: nvisy_agent::pipeline::NerPipeline
//! [`NerPipeline::new`]: nvisy_agent::pipeline::NerPipeline::new
//! [`NerPipeline::run`]: nvisy_agent::pipeline::NerPipeline::run
//! [`Pipeline::reset`]: nvisy_agent::pipeline::NerPipeline::reset
//! [`new`]: LlmRecognizer::new
//! [`from_pipeline`]: LlmRecognizer::from_pipeline

mod context;
mod params;

use async_trait::async_trait;
use nvisy_agent::agent::DetectionConfig;
use nvisy_agent::pipeline::NerPipeline;
use nvisy_core::Result;
use nvisy_ontology::entity::Entities;

pub use self::context::LlmContext;
pub use self::params::LlmDetection;
use crate::detection::Recognizer;

/// LLM-backed entity recognizer.
///
/// Wraps an internally-built [`NerPipeline`] and exposes it via the
/// [`Recognizer`] trait. Per-call detection hints translate into the
/// rig [`DetectionConfig`] passed to [`NerPipeline::run`].
///
/// [`NerPipeline`]: nvisy_agent::pipeline::NerPipeline
/// [`NerPipeline::run`]: nvisy_agent::pipeline::NerPipeline::run
pub struct LlmRecognizer {
    pipeline: NerPipeline,
}

impl LlmRecognizer {
    /// Build a recognizer from an [`LlmDetection`] config bundle.
    ///
    /// `cfg.verify_pass` toggles the second-pass refinement
    /// verifier. `false` runs localization-only verification (one
    /// LLM call per span); `true` enables the two-pass verifier
    /// reusing the same `cfg.agent` config (two LLM calls per
    /// span).
    ///
    /// # Errors
    ///
    /// Returns an error if the rig agent or verifier cannot be
    /// constructed (bad provider, invalid config).
    pub fn new(cfg: LlmDetection) -> Result<Self> {
        let LlmDetection {
            provider,
            agent,
            verify_pass,
            unresolved_policy,
        } = cfg;
        let verifier_config = verify_pass.then(|| agent.clone());
        let pipeline = NerPipeline::new(&provider, agent, verifier_config, unresolved_policy)
            .map_err(|e| nvisy_core::Error::runtime(e.to_string(), "llm", false))?;
        Ok(Self::from_pipeline(pipeline))
    }

    /// Build from a pre-assembled [`NerPipeline`].
    ///
    /// Escape hatch for callers that already own a pipeline (e.g.
    /// to share an instance across recognizers). Prefer [`new`] for
    /// ordinary use.
    ///
    /// [`NerPipeline`]: nvisy_agent::pipeline::NerPipeline
    /// [`new`]: Self::new
    pub fn from_pipeline(pipeline: NerPipeline) -> Self {
        Self { pipeline }
    }

    fn build_config(ctx: &LlmContext) -> DetectionConfig {
        DetectionConfig {
            entity_kinds: ctx.entities.clone().unwrap_or_default(),
            confidence_threshold: ctx.score_threshold,
            system_prompt: None,
        }
    }
}

#[async_trait]
impl Recognizer for LlmRecognizer {
    type Context = LlmContext;

    #[tracing::instrument(
        skip_all,
        fields(
            text_len = ctx.text.len(),
            correlation_id = ctx.correlation_id.as_ref().map(|id| id.to_string()),
        ),
    )]
    async fn run(&self, ctx: &LlmContext) -> Result<Entities> {
        let config = Self::build_config(ctx);
        self.pipeline
            .run(&ctx.text, &config)
            .await
            .map_err(|e| nvisy_core::Error::runtime(e.to_string(), "llm", false))
    }

    /// Clears coreference state at document boundaries by
    /// delegating to [`Pipeline::reset`].
    ///
    /// [`Pipeline::reset`]: nvisy_agent::pipeline::NerPipeline::reset
    async fn reset(&self) {
        self.pipeline.reset().await;
    }
}
