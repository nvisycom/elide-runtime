//! [`LlmRecognizer`]: LLM-driven NER over `nvisy_rig::agent::NerAgent`
//! plus `nvisy_rig::agent::NerVerifier`.
//!
//! Two-stage flow per `recognize` call:
//!
//! 1. **Detect.** Ask the LLM agent for [`NerCandidate`]s — surface
//!    forms + context windows, no offsets.
//! 2. **Verify.** Hand the candidates to the verifier, which
//!    localizes each one into a byte range in the source text via
//!    `context` search and (optionally, when the recognizer was
//!    constructed with [`with_refinement`]) prompts the LLM again
//!    to confirm/correct/reject.
//!
//! Per-call detection hints from [`DetectionContext`] (`entities`
//! kind allowlist, `score_threshold`) flow into the agent's rig
//! `DetectionConfig`. Coreference state (`KnownNerEntity`)
//! accumulates between successive calls and resets at document
//! boundaries via [`reset`].
//!
//! [`DetectionContext`]: crate::DetectionContext
//! [`NerCandidate`]: nvisy_rig::agent::NerCandidate
//! [`with_refinement`]: LlmRecognizer::with_refinement
//! [`reset`]: Recognizer::reset

use async_trait::async_trait;
use nvisy_ontology::entity::Entities;
use nvisy_rig::agent::{
    AgentConfig, AgentProvider, DetectionConfig, KnownNerEntity, NerAgent, NerContext, NerVerifier,
};
use tokio::sync::Mutex;

use crate::error::{Error, Result};
use crate::{DetectionContext, Recognizer};

/// LLM-backed entity recognizer.
pub struct LlmRecognizer {
    agent: NerAgent,
    verifier: NerVerifier,
    state: Mutex<Vec<KnownNerEntity>>,
}

impl LlmRecognizer {
    /// Construct from a pre-built [`NerAgent`] with a
    /// localization-only verifier. Per-call detection hints
    /// (entity-kind allowlist, score threshold) flow through
    /// [`DetectionContext`] at recognize time.
    ///
    /// [`DetectionContext`]: crate::DetectionContext
    pub fn new(agent: NerAgent) -> Self {
        Self {
            agent,
            verifier: NerVerifier::new(),
            state: Mutex::new(Vec::new()),
        }
    }

    /// Add a second-pass LLM refiner to the verifier. The refiner
    /// re-checks each localized candidate against its snippet and
    /// may confirm/correct/reject.
    pub fn with_refinement(
        mut self,
        provider: &AgentProvider,
        config: AgentConfig,
    ) -> Result<Self> {
        self.verifier = self
            .verifier
            .with_refinement(provider, config)
            .map_err(|e| Error::Recognizer {
                name: "llm".into(),
                cause: e.to_string(),
            })?;
        Ok(self)
    }

    /// Replace the default verifier outright (advanced).
    pub fn with_verifier(mut self, verifier: NerVerifier) -> Self {
        self.verifier = verifier;
        self
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

        // 1. Detect — build an NerContext with accumulated coreference.
        let known = self.state.lock().await.clone();
        let nlp_ctx = NerContext::with_known(ctx.text, known);

        let candidates =
            self.agent
                .detect(&nlp_ctx, &config)
                .await
                .map_err(|e| Error::Recognizer {
                    name: "llm".into(),
                    cause: e.to_string(),
                })?;

        // 2. Verify — localize and (optionally) refine.
        let entities = self
            .verifier
            .verify(ctx.text, candidates.clone())
            .await
            .map_err(|e| Error::Recognizer {
                name: "llm".into(),
                cause: e.to_string(),
            })?;

        // 3. Update coreference state from candidates that
        //    *survived* verification. We can't pass Entities into
        //    NerContext::merge directly (it wants candidates), so
        //    filter the original candidate list by the set of
        //    entity_ids that made it through.
        let surviving_ids: std::collections::HashSet<&str> = entities
            .iter()
            .filter_map(|e| e.entity_id.as_deref())
            .collect();
        let surviving: Vec<_> = candidates
            .into_iter()
            .filter(|c| surviving_ids.contains(c.entity_id.as_str()))
            .collect();
        {
            let mut state = self.state.lock().await;
            let mut merge_ctx = NerContext::with_known(ctx.text, std::mem::take(&mut *state));
            merge_ctx.merge(surviving);
            *state = merge_ctx.known_entities;
        }

        Ok(entities)
    }

    /// Clears coreference state at document boundaries.
    async fn reset(&self) {
        self.state.lock().await.clear();
    }
}
