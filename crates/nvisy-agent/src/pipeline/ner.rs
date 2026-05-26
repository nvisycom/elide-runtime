//! [`NerPipeline`]: detect → verify → coreference-merge.
//!
//! Three responsibilities behind one `run()` call:
//!
//! 1. Ask the [`NerAgent`] for [`NerCandidate`]s, threading the
//!    current coreference state as `NerContext::known_entities`.
//! 2. Hand the candidates to the [`NerVerifyAgent`] which localizes
//!    them into byte offsets and optionally LLM-refines them.
//! 3. Filter the original candidate list by the set of
//!    `entity_id`s that survived verification and merge the
//!    survivors into coreference state. Rejected candidates don't
//!    pollute the next call's prompt.
//!
//! The verifier is always present (every consumer wants verified
//! output). The verifier's optional refinement pass — a second LLM
//! call that confirms/corrects/rejects each localized candidate —
//! is configured on the verifier itself via
//! [`NerVerifyAgent::with_refinement`].
//!
//! [`NerAgent`]: crate::agent::ner::NerAgent
//! [`NerCandidate`]: crate::agent::ner::NerCandidate
//! [`NerVerifyAgent`]: crate::agent::ner::NerVerifyAgent
//! [`NerVerifyAgent::with_refinement`]: crate::agent::ner::NerVerifyAgent::with_refinement

use std::collections::HashSet;
use std::mem;

use derive_builder::Builder;
use nvisy_core::Result;
use nvisy_ontology::entity::Entities;
use nvisy_ontology::modality::Text;
use tokio::sync::Mutex;

use crate::agent::ner::{
    KnownNerEntity, NerAgent, NerContext, NerVerifyAgent, UnresolvedCandidatePolicy,
};
use crate::agent::{AgentConfig, AgentProvider, LlmNerContext, UsageStats};

/// Composed NER pipeline.
///
/// Holds the detection [`NerAgent`], the [`NerVerifyAgent`] that
/// localizes candidates into byte offsets, and the cross-call
/// coreference state shared between successive `run()` calls. Use
/// [`reset`] at document boundaries to clear the state.
///
/// Construct via [`new`]; the agent and verifier are built
/// internally from the LLM provider + configs, and state is
/// initialised empty.
///
/// [`reset`]: Self::reset
/// [`new`]: Self::new
/// [`NerAgent`]: crate::agent::ner::NerAgent
/// [`NerVerifyAgent`]: crate::agent::ner::NerVerifyAgent
#[derive(Builder)]
#[builder(
    name = "NerPipelineBuilder",
    pattern = "owned",
    build_fn(error = "NerPipelineBuilderError")
)]
pub struct NerPipeline {
    #[builder(setter(custom))]
    agent: NerAgent,
    #[builder(setter(custom))]
    verifier: NerVerifyAgent,
    #[builder(setter(skip), default)]
    state: Mutex<Vec<KnownNerEntity>>,
}

impl NerPipelineBuilder {
    /// Attach the detection agent. Required.
    pub fn with_agent(mut self, agent: NerAgent) -> Self {
        self.agent = Some(agent);
        self
    }

    /// Attach the verifier. Required. Localization always runs;
    /// the verifier's optional second-pass LLM refinement is
    /// configured on the verifier itself via
    /// [`NerVerifyAgent::with_refinement`].
    ///
    /// [`NerVerifyAgent::with_refinement`]: crate::agent::ner::NerVerifyAgent::with_refinement
    pub fn with_verifier(mut self, verifier: NerVerifyAgent) -> Self {
        self.verifier = Some(verifier);
        self
    }
}

/// Error returned by [`NerPipelineBuilder::build`] when a required
/// component is missing. Crate-internal — `NerPipeline::new` wraps
/// the builder for external callers.
#[derive(Debug, thiserror::Error)]
#[error("NerPipeline build failed: {0}")]
pub(crate) struct NerPipelineBuilderError(String);

impl From<derive_builder::UninitializedFieldError> for NerPipelineBuilderError {
    fn from(err: derive_builder::UninitializedFieldError) -> Self {
        Self(format!("missing required component `{}`", err.field_name()))
    }
}

impl NerPipeline {
    /// Build a pipeline from an LLM provider plus agent configs.
    ///
    /// `agent_config` drives the detection-pass agent. `verifier_config`
    /// is `Some` to enable the two-pass refinement verifier with the
    /// carried config (two LLM calls per span), or `None` for
    /// localization-only verification (one LLM call per span).
    /// `unresolved_policy` controls what the verifier does with
    /// candidates that can't be uniquely localized in the source.
    ///
    /// # Errors
    ///
    /// Returns an error if the detection agent or (when requested)
    /// the verifier agent cannot be constructed.
    pub fn new(
        provider: &AgentProvider,
        agent_config: AgentConfig,
        verifier_config: Option<AgentConfig>,
        unresolved_policy: UnresolvedCandidatePolicy,
    ) -> Result<Self> {
        let agent = NerAgent::new(provider, agent_config)?;
        let verifier = match verifier_config {
            Some(cfg) => NerVerifyAgent::new().with_refinement(provider, cfg)?,
            None => NerVerifyAgent::new(),
        };
        let verifier = verifier.with_unresolved_policy(unresolved_policy);
        Self::builder()
            .with_agent(agent)
            .with_verifier(verifier)
            .build()
            .map_err(|e| nvisy_core::Error::validation(e.to_string(), "ner-pipeline"))
    }

    /// Start building a pipeline.
    ///
    /// Crate-internal — external callers go through [`new`].
    ///
    /// [`new`]: Self::new
    pub(crate) fn builder() -> NerPipelineBuilder {
        NerPipelineBuilder::default()
    }

    /// Run the pipeline once: detect candidates, verify them into
    /// [`Entities`], and merge surviving candidates into the
    /// coreference state for the next call.
    pub async fn run(&self, text: &str, config: &LlmNerContext) -> Result<Entities<Text>> {
        // 1. Detect — agent sees the accumulated known entities so
        //    it can reuse stable entity_ids for coreferent mentions.
        let known = self.state.lock().await.clone();
        let nlp_ctx = NerContext::with_known(text, known);
        let candidates = self.agent.detect(&nlp_ctx, config).await?;

        // 2. Verify — localize + optionally LLM-refine.
        let entities = self.verifier.verify(text, candidates.clone()).await?;

        // 3. Merge coreference state from candidates that *survived*
        //    verification. Rejected candidates don't pollute next
        //    call's prompt. Candidates without entity_id can't
        //    participate in coreference and are dropped from the
        //    merge regardless (NerContext::merge also skips them).
        let surviving_ids: HashSet<&str> = entities
            .iter()
            .filter_map(|e| e.entity_id.as_deref())
            .collect();
        let surviving: Vec<_> = candidates
            .into_iter()
            .filter(|c| {
                c.entity_id
                    .as_deref()
                    .is_some_and(|id| surviving_ids.contains(id))
            })
            .collect();

        let mut state = self.state.lock().await;
        let mut merge_ctx = NerContext::with_known(text, mem::take(&mut *state));
        merge_ctx.merge(surviving);
        *state = merge_ctx.known_entities;

        Ok(entities)
    }
}

impl NerPipeline {
    /// Clear per-document state and reset cumulative usage
    /// counters. Call at document boundaries so coreference and
    /// per-document accounting don't bleed across runs.
    pub async fn reset(&self) {
        self.state.lock().await.clear();
        self.agent.tracker().reset();
        if let Some(tracker) = self.verifier.tracker() {
            tracker.reset();
        }
    }

    /// Cumulative token usage since the last [`reset`], summed
    /// across the detect agent and (when configured) the verifier
    /// agent.
    ///
    /// [`reset`]: Self::reset
    pub fn usage(&self) -> UsageStats {
        let mut stats = self.agent.tracker().snapshot();
        if let Some(tracker) = self.verifier.tracker() {
            stats += tracker.snapshot();
        }
        stats
    }
}
