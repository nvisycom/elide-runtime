//! [`NerPipeline`]: detect → verify → coreference-merge.
//!
//! Three responsibilities behind one `run()` call:
//!
//! 1. Ask the [`NerAgent`] for [`NerCandidate`]s, threading the
//!    current coreference state as `NerContext::known_entities`.
//! 2. Hand the candidates to the [`NerVerifier`] which localizes
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
//! [`NerVerifier::with_refinement`].
//!
//! [`NerAgent`]: crate::agent::NerAgent
//! [`NerCandidate`]: crate::agent::NerCandidate
//! [`NerVerifier`]: crate::agent::NerVerifier
//! [`NerVerifier::with_refinement`]: crate::agent::NerVerifier::with_refinement

use std::collections::HashSet;
use std::mem;

use async_trait::async_trait;
use derive_builder::Builder;
use nvisy_core::Result;
use nvisy_ontology::entity::Entities;
use tokio::sync::Mutex;

use super::Pipeline;
use crate::agent::{DetectionConfig, KnownNerEntity, NerAgent, NerContext, NerVerifier};

/// Composed NER pipeline.
///
/// Holds the detection [`NerAgent`], the [`NerVerifier`] that
/// localizes candidates into byte offsets, and the cross-call
/// coreference state shared between successive `run()` calls. Use
/// [`Pipeline::reset`] at document boundaries to clear the state.
///
/// Construct via [`NerPipeline::builder`]; the agent and verifier
/// are required, state is initialised empty.
///
/// [`Pipeline::reset`]: super::Pipeline::reset
/// [`NerPipeline::builder`]: Self::builder
/// [`NerAgent`]: crate::agent::NerAgent
/// [`NerVerifier`]: crate::agent::NerVerifier
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
    verifier: NerVerifier,
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
    /// [`NerVerifier::with_refinement`].
    ///
    /// [`NerVerifier::with_refinement`]: crate::agent::NerVerifier::with_refinement
    pub fn with_verifier(mut self, verifier: NerVerifier) -> Self {
        self.verifier = Some(verifier);
        self
    }
}

/// Error returned by [`NerPipelineBuilder::build`] when a required
/// component is missing.
#[derive(Debug, thiserror::Error)]
#[error("NerPipeline build failed: {0}")]
pub struct NerPipelineBuilderError(String);

impl From<derive_builder::UninitializedFieldError> for NerPipelineBuilderError {
    fn from(err: derive_builder::UninitializedFieldError) -> Self {
        Self(format!("missing required component `{}`", err.field_name()))
    }
}

impl NerPipeline {
    /// Start building a pipeline.
    pub fn builder() -> NerPipelineBuilder {
        NerPipelineBuilder::default()
    }

    /// Run the pipeline once: detect candidates, verify them into
    /// [`Entities`], and merge surviving candidates into the
    /// coreference state for the next call.
    pub async fn run(&self, text: &str, config: &DetectionConfig) -> Result<Entities> {
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

#[async_trait]
impl Pipeline for NerPipeline {
    async fn reset(&self) {
        self.state.lock().await.clear();
    }
}
