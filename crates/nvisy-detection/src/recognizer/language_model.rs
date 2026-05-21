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
//! Prefer the config-driven constructors — [`from_llm`] for
//! localization-only verification and [`from_llm_refined`] for the
//! two-pass verifier — so the pipeline is built internally and the
//! caller never touches rig agent types. [`from_pipeline`] is
//! retained as an escape hatch for callers that need to customize
//! the verifier (e.g. [`UnresolvedCandidatePolicy`]) before
//! attaching it.
//!
//! [`DetectionContext`]: crate::DetectionContext
//! [`NerPipeline`]: nvisy_rig::pipeline::NerPipeline
//! [`NerPipeline::run`]: nvisy_rig::pipeline::NerPipeline::run
//! [`Pipeline::reset`]: nvisy_rig::pipeline::Pipeline::reset
//! [`from_llm`]: LlmRecognizer::from_llm
//! [`from_llm_refined`]: LlmRecognizer::from_llm_refined
//! [`from_pipeline`]: LlmRecognizer::from_pipeline
//! [`UnresolvedCandidatePolicy`]: nvisy_rig::agent::UnresolvedCandidatePolicy

use async_trait::async_trait;
use nvisy_ontology::entity::Entities;
use nvisy_rig::agent::{AgentConfig, AgentProvider, DetectionConfig, NerAgent, NerVerifier};
use nvisy_rig::pipeline::{NerPipeline, Pipeline};

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
    /// Build a recognizer with a localization-only verifier (no
    /// second-pass LLM refinement).
    ///
    /// One LLM call per text span: the detection agent. Verification
    /// resolves each candidate's surface form into byte offsets but
    /// does not re-prompt the model.
    ///
    /// # Errors
    ///
    /// Returns an error if the rig agent cannot be constructed (bad
    /// provider, invalid config).
    pub fn from_llm(provider: &AgentProvider, config: AgentConfig) -> Result<Self> {
        let agent = NerAgent::new(provider, config).map_err(Self::map_build_err)?;
        let verifier = NerVerifier::new();
        Self::assemble(agent, verifier)
    }

    /// Build a recognizer with a two-pass verifier: localize, then
    /// LLM-refine each localized candidate.
    ///
    /// Two LLM calls per text span: the detection agent followed by
    /// the verifier's refinement prompt. `detect` and `verify`
    /// configs are independent so the verifier can use a
    /// cheaper/stricter model than detection.
    ///
    /// # Errors
    ///
    /// Returns an error if either rig agent cannot be constructed.
    pub fn from_llm_refined(
        provider: &AgentProvider,
        detect: AgentConfig,
        verify: AgentConfig,
    ) -> Result<Self> {
        let agent = NerAgent::new(provider, detect).map_err(Self::map_build_err)?;
        let verifier = NerVerifier::new()
            .with_refinement(provider, verify)
            .map_err(Self::map_build_err)?;
        Self::assemble(agent, verifier)
    }

    /// Build from a pre-assembled [`NerPipeline`].
    ///
    /// Escape hatch for callers that need to customize the verifier
    /// (e.g. [`UnresolvedCandidatePolicy`]) or share a pipeline
    /// instance across recognizers. Prefer [`from_llm`] /
    /// [`from_llm_refined`] for ordinary use.
    ///
    /// [`NerPipeline`]: nvisy_rig::pipeline::NerPipeline
    /// [`UnresolvedCandidatePolicy`]: nvisy_rig::agent::UnresolvedCandidatePolicy
    /// [`from_llm`]: Self::from_llm
    /// [`from_llm_refined`]: Self::from_llm_refined
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
