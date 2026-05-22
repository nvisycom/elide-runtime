//! [`LlmRecognizer`]: adapts an [`NerPipeline`] to the
//! [`Recognizer`] trait.
//!
//! All NER-specific orchestration — detect, verify, coreference
//! merge — lives on [`NerPipeline`] in nvisy-rig. This recognizer
//! is a thin adapter: translate [`DetectionContext`] into a rig
//! [`DetectionConfig`], call [`NerPipeline::run`], and forward
//! `reset()` to [`Pipeline::reset`].
//!
//! # Construction
//!
//! [`new`] consumes a single [`LlmDetection`] bundle and builds
//! everything internally. The presence of [`LlmDetection::verifier`]
//! decides whether the pipeline gets a localization-only verifier
//! or the two-pass refinement verifier.
//!
//! [`from_pipeline`] is retained as an escape hatch for callers
//! that need to customize the verifier (e.g.
//! [`UnresolvedCandidatePolicy`]) before attaching it.
//!
//! [`DetectionContext`]: crate::DetectionContext
//! [`NerPipeline`]: nvisy_rig::pipeline::NerPipeline
//! [`NerPipeline::run`]: nvisy_rig::pipeline::NerPipeline::run
//! [`Pipeline::reset`]: nvisy_rig::pipeline::Pipeline::reset
//! [`new`]: LlmRecognizer::new
//! [`from_pipeline`]: LlmRecognizer::from_pipeline
//! [`UnresolvedCandidatePolicy`]: nvisy_rig::agent::UnresolvedCandidatePolicy

mod params;

use async_trait::async_trait;
use nvisy_ontology::entity::Entities;
use nvisy_rig::agent::{DetectionConfig, NerAgent, NerVerifier};
use nvisy_rig::pipeline::{NerPipeline, Pipeline};

pub use self::params::LlmDetection;
use crate::error::{Error, Result};
use crate::{DetectionContext, Recognizer};

/// LLM-backed entity recognizer.
///
/// Wraps an internally-built [`NerPipeline`] and exposes it via the
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
    /// Build a recognizer from an [`LlmDetection`] config bundle.
    ///
    /// `cfg.verifier` toggles the second-pass verifier: `None` gives
    /// a localization-only verifier (one LLM call per span), `Some`
    /// enables the two-pass refinement verifier with the carried
    /// agent config (two LLM calls per span; the verifier may use a
    /// cheaper/stricter model than detection).
    ///
    /// # Errors
    ///
    /// Returns an error if the rig agent or verifier cannot be
    /// constructed (bad provider, invalid config).
    pub fn new(cfg: LlmDetection) -> Result<Self> {
        let LlmDetection {
            provider,
            agent,
            verifier,
        } = cfg;
        let agent = NerAgent::new(&provider, agent).map_err(Self::map_build_err)?;
        let verifier = match verifier {
            Some(verifier_cfg) => NerVerifier::new()
                .with_refinement(&provider, verifier_cfg)
                .map_err(Self::map_build_err)?,
            None => NerVerifier::new(),
        };
        Self::assemble(agent, verifier)
    }

    /// Build from a pre-assembled [`NerPipeline`].
    ///
    /// Escape hatch for callers that need to customize the verifier
    /// (e.g. [`UnresolvedCandidatePolicy`]) or share a pipeline
    /// instance across recognizers. Prefer [`new`] for ordinary use.
    ///
    /// [`NerPipeline`]: nvisy_rig::pipeline::NerPipeline
    /// [`UnresolvedCandidatePolicy`]: nvisy_rig::agent::UnresolvedCandidatePolicy
    /// [`new`]: Self::new
    pub fn from_pipeline(pipeline: NerPipeline) -> Self {
        Self { pipeline }
    }

    fn assemble(agent: NerAgent, verifier: NerVerifier) -> Result<Self> {
        let pipeline = NerPipeline::builder()
            .with_agent(agent)
            .with_verifier(verifier)
            .build()
            .map_err(|e| Error::Recognizer {
                name: "llm".into(),
                cause: e.to_string(),
            })?;
        Ok(Self::from_pipeline(pipeline))
    }

    fn map_build_err(err: nvisy_core::Error) -> Error {
        Error::Recognizer {
            name: "llm".into(),
            cause: err.to_string(),
        }
    }

    /// Build the rig per-call [`DetectionConfig`] from the per-call
    /// [`DetectionContext`].
    ///
    /// [`DetectionContext`]: crate::DetectionContext
    fn build_config(ctx: &DetectionContext) -> DetectionConfig {
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
    async fn run(&self, ctx: &DetectionContext) -> Result<Entities> {
        let config = Self::build_config(ctx);
        self.pipeline
            .run(&ctx.text, &config)
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
