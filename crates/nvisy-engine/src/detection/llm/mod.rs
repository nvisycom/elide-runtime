//! [`LlmRecognizer`]: adapts an [`NerPipeline`] to the
//! [`Recognizer`] trait.
//!
//! All NER-specific orchestration — detect, verify, coreference
//! merge — lives on [`NerPipeline`] in nvisy-agent. This recognizer
//! is a thin adapter that forwards the [`LlmNerContext`] derived
//! from a [`DetectionContext`] into [`NerPipeline::run`], and
//! forwards `reset()` to [`Pipeline::reset`].
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

mod params;

use async_trait::async_trait;
use nvisy_agent::agent::LlmNerContext;
use nvisy_agent::pipeline::NerPipeline;
use nvisy_core::Result;
use nvisy_ontology::entity::Entity;
use nvisy_ontology::modality::Text;

pub use self::params::LlmDetection;
use crate::detection::{DetectionContext, Recognizer};

/// LLM-backed entity recognizer.
///
/// Wraps an internally-built [`NerPipeline`] and exposes it via the
/// [`Recognizer`] trait. The per-call configuration is
/// [`LlmNerContext`] directly — derived from
/// [`DetectionContext`] via [`From`].
///
/// [`NerPipeline`]: nvisy_agent::pipeline::NerPipeline
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
        let verifier_config = cfg.verify_pass.then(|| cfg.agent.clone());
        let pipeline = NerPipeline::new(
            &cfg.provider,
            cfg.agent,
            verifier_config,
            cfg.unresolved_policy,
        )
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
}

#[async_trait]
impl Recognizer for LlmRecognizer {
    type Context = LlmNerContext;

    #[tracing::instrument(
        skip_all,
        fields(
            text_len = text.len(),
            correlation_id = ctx.correlation_id.as_ref().map(|id| id.to_string()),
        ),
    )]
    async fn run(&self, text: &str, ctx: &LlmNerContext) -> Result<Vec<Entity<Text>>> {
        self.pipeline
            .run(text, ctx)
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

impl From<&DetectionContext> for LlmNerContext {
    fn from(ctx: &DetectionContext) -> Self {
        Self {
            entity_kinds: ctx.entities.clone().unwrap_or_default(),
            confidence_threshold: ctx.score_threshold,
            system_prompt: None,
            correlation_id: ctx.correlation_id,
        }
    }
}
