//! Named Entity Recognition (NER) agent for textual PII/entity detection.
//!
//! [`NerAgent`] wraps a [`BaseAgent`] with NER-specific prompts. It
//! is a pure LLM agent (no tools) that analyses text and returns
//! [`NerCandidate`]s — unresolved entity descriptions that a
//! downstream [`NerVerifyAgent`] localizes into the source text and
//! lifts into [`Entity`] values.
//!
//! [`BaseAgent`]: super::BaseAgent
//! [`NerVerifyAgent`]: crate::agent::ner::NerVerifyAgent
//! [`Entity`]: nvisy_ontology::entity::Entity

mod context;
mod output;
mod prompt;

use nvisy_core::Result;
use uuid::Uuid;

pub use self::context::NerContext;
use self::output::NerCandidates;
pub use self::output::{KnownNerEntity, NerCandidate};
use self::prompt::{NER_SYSTEM_PROMPT, NerPromptBuilder};
use crate::agent::base::{BaseAgent, UsageTracker};
use crate::agent::{AgentConfig, AgentProvider, LlmNerContext};

const TARGET: &str = "nvisy_agent::agent::ner";

/// Agent for textual PII/entity detection using LLM-based NER.
///
/// # Workflow
///
/// 1. Caller passes a [`NerContext`] and a [`LlmNerContext`] to
///    [`detect`].
/// 2. The agent builds a user prompt via `NerPromptBuilder` that
///    specifies entity types, confidence thresholds, and known
///    entities.
/// 3. Structured output is parsed into `Vec<NerCandidate>`.
///
/// Localization of candidates into byte offsets and construction
/// of [`Entity`] values is the responsibility of [`NerVerifyAgent`].
/// Coreference state across successive calls is also a verifier
/// concern; this agent is stateless.
///
/// [`detect`]: Self::detect
/// [`Entity`]: nvisy_ontology::entity::Entity
/// [`NerVerifyAgent`]: crate::agent::ner::NerVerifyAgent
pub struct NerAgent {
    base: BaseAgent,
}

impl NerAgent {
    /// Create a new NER agent. The HTTP client is built internally
    /// from `config.max_retries` and otherwise-default settings.
    pub fn new(provider: &AgentProvider, mut config: AgentConfig) -> Result<Self> {
        config
            .preamble
            .get_or_insert_with(|| NER_SYSTEM_PROMPT.into());
        let base = BaseAgent::builder(provider, config)
            .build()
            .map_err(crate::error::convert)?;
        Ok(Self { base })
    }

    /// Unique identifier for this agent instance (UUIDv7).
    pub fn id(&self) -> Uuid {
        self.base.id()
    }

    /// Access the usage tracker for this agent's LLM calls.
    pub fn tracker(&self) -> &UsageTracker {
        self.base.tracker()
    }

    /// The model name used by this agent.
    pub fn model_name(&self) -> &str {
        self.base.model_name()
    }

    /// Detect entity candidates in text.
    ///
    /// When [`NerContext::known_entities`] is non-empty the LLM is
    /// instructed to reuse their `entity_id` values for coreferent
    /// mentions, enabling cross-chunk coreference resolution.
    /// Returned candidates carry no offsets; pass them to a
    /// [`NerVerifyAgent`] to localize.
    ///
    /// [`NerVerifyAgent`]: crate::agent::ner::NerVerifyAgent
    #[tracing::instrument(target = TARGET, skip_all, fields(text_len = ctx.text.len()))]
    pub async fn detect(
        &self,
        ctx: &NerContext<'_>,
        config: &LlmNerContext,
    ) -> Result<Vec<NerCandidate>> {
        let prompt = NerPromptBuilder::new(config, &ctx.known_entities).build(ctx.text);

        tracing::debug!(
            target: TARGET,
            prompt_len = prompt.len(),
            entity_kinds = config.entity_kinds.len(),
            known = ctx.known_entities.len(),
            "built ner prompt"
        );

        let result: NerCandidates = self
            .base
            .prompt_structured(&prompt)
            .await
            .map_err(crate::error::convert)?;

        tracing::info!(
            target: TARGET,
            candidate_count = result.entities.len(),
            "ner detection complete"
        );

        Ok(result.entities)
    }
}
