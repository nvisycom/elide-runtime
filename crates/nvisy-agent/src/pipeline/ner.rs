//! [`LlmNerPipeline`]: optional detect + optional whole-audit verify.
//!
//! Both agents are independently optional so operators can pick
//! any of: detect-only, verify-only (over entities other
//! recognizers produced), both, or neither (the recognizer
//! becomes a no-op).
//!
//! 1. If a detect agent is configured, ask [`NerAgent`] to detect
//!    entities. Otherwise start with an empty entity list.
//! 2. If a verifier is configured, hand the entities to
//!    [`NerVerifyAgent`] for whole-audit confirm/reject/adjust.
//!    Otherwise return the detect output unchanged.
//!
//! [`NerAgent`]: crate::agent::ner::NerAgent
//! [`NerVerifyAgent`]: crate::agent::ner::NerVerifyAgent

use nvisy_core::entity::Entity;
use nvisy_core::modality::Text;
use nvisy_core::{Error, Result};

use crate::agent::ner::{NerAgent, NerVerifyAgent, UnresolvedCandidatePolicy};
use crate::agent::{AgentConfig, AgentProvider, LlmNerContext, UsageStats};

/// Composed NER pipeline.
///
/// Construct via [`new`]; both the detect and verify agents are
/// optional. Built internally from the LLM provider + configs.
///
/// [`new`]: Self::new
pub struct LlmNerPipeline {
    agent: Option<NerAgent>,
    verifier: Option<NerVerifyAgent>,
}

impl LlmNerPipeline {
    /// Build a pipeline from an LLM provider plus agent configs.
    ///
    /// `agent_config` drives the detect-pass agent when `Some`;
    /// `verifier_config` drives the whole-audit verifier when
    /// `Some`. At least one must be `Some` — a pipeline with
    /// neither would never produce or process entities.
    /// `unresolved_policy` controls how the detect-pass localizer
    /// handles candidates that can't be uniquely placed in the
    /// source.
    ///
    /// # Errors
    ///
    /// Returns an error if both agents are `None`, or if any
    /// requested agent cannot be constructed.
    pub fn new(
        provider: &AgentProvider,
        agent_config: Option<AgentConfig>,
        verifier_config: Option<AgentConfig>,
        unresolved_policy: UnresolvedCandidatePolicy,
    ) -> Result<Self> {
        if agent_config.is_none() && verifier_config.is_none() {
            return Err(Error::validation(
                "LlmNerPipeline requires at least one of detect or verify to be configured",
                "ner-pipeline",
            ));
        }
        let agent = match agent_config {
            Some(cfg) => {
                Some(NerAgent::new(provider, cfg)?.with_unresolved_policy(unresolved_policy))
            }
            None => None,
        };
        let verifier = match verifier_config {
            Some(cfg) => Some(NerVerifyAgent::new(provider, cfg)?),
            None => None,
        };
        Ok(Self { agent, verifier })
    }

    /// Run the pipeline once. With both agents configured: detect
    /// then verify. With detect only: return detect output. With
    /// verify only: run verify against an empty input (returning
    /// no entities — verify in isolation only makes sense when
    /// called from a driver that supplies external entities, which
    /// the current `run` shape doesn't expose; this path is here
    /// for completeness and will be revisited if the engine
    /// driver grows a separate verify entrypoint).
    pub async fn run(&self, text: &str, config: &LlmNerContext) -> Result<Vec<Entity<Text>>> {
        let entities = match &self.agent {
            Some(a) => a.detect(text, config).await?,
            None => Vec::new(),
        };
        match &self.verifier {
            Some(v) => v.verify(text, entities).await,
            None => Ok(entities),
        }
    }

    /// Reset cumulative usage counters. Call at document boundaries
    /// so per-document accounting doesn't bleed across runs.
    pub async fn reset(&self) {
        if let Some(a) = &self.agent {
            a.tracker().reset();
        }
        if let Some(v) = &self.verifier {
            v.tracker().reset();
        }
    }

    /// Cumulative token usage since the last [`reset`], summed
    /// across the configured passes.
    ///
    /// [`reset`]: Self::reset
    pub fn usage(&self) -> UsageStats {
        let mut stats = UsageStats::default();
        if let Some(a) = &self.agent {
            stats += a.tracker().snapshot();
        }
        if let Some(v) = &self.verifier {
            stats += v.tracker().snapshot();
        }
        stats
    }
}
