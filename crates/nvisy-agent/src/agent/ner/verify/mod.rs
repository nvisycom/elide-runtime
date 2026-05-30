//! [`NerVerifyAgent`]: whole-audit LLM filter over already-built
//! [`Entity<Text>`] values.
//!
//! Takes the merged entities produced by all upstream recognizers
//! (pattern, NER, annotate) plus the source text, prompts the LLM
//! to vote per entity, and returns the survivors. The verdict
//! schema is the shared [`VerificationOutput`]:
//!
//! - **Confirmed** (entity absent from the LLM's response) — kept
//!   unchanged.
//! - **Rejected** — dropped.
//! - **Corrected** — kept with optional adjustments to category /
//!   entity kind / confidence, and stamped with
//!   [`RefinementMethod::ModelVerification`]. `value` and `bbox`
//!   fields in the verdict are ignored (the entity's location and
//!   thus surface form are frozen from the original recognizer).
//!
//! Entities are passed to the LLM by index; the LLM returns those
//! indices in its verdict.
//!
//! [`Entity<Text>`]: nvisy_ontology::entity::Entity
//! [`VerificationOutput`]: crate::agent::base::VerificationOutput
//! [`RefinementMethod::ModelVerification`]: nvisy_ontology::entity::RefinementMethod::ModelVerification

mod prompt;

use nvisy_core::Result;
use nvisy_ontology::entity::Entity;
use nvisy_ontology::modality::Text;

use self::prompt::{NER_VERIFIER_SYSTEM_PROMPT, NerVerifyPromptBuilder};
use crate::agent::base::{BaseAgent, UsageTracker, VerificationOutput};
use crate::agent::{AgentConfig, AgentProvider};

const TARGET: &str = "nvisy_agent::agent::ner::verify";

/// Whole-audit LLM verifier for [`Entity<Text>`] values.
///
/// Stateless — constructed once per pipeline and reused across
/// calls.
pub struct NerVerifyAgent {
    agent: BaseAgent,
}

impl NerVerifyAgent {
    /// Build a verifier from an LLM provider + agent config.
    pub fn new(provider: &AgentProvider, mut config: AgentConfig) -> Result<Self> {
        config
            .preamble
            .get_or_insert_with(|| NER_VERIFIER_SYSTEM_PROMPT.into());
        let agent = BaseAgent::builder(provider, config)
            .build()
            .map_err(crate::error::convert)?;
        Ok(Self { agent })
    }

    /// Access the usage tracker for this verifier's LLM calls.
    pub fn tracker(&self) -> &UsageTracker {
        self.agent.tracker()
    }

    /// UUID of the verifier agent.
    pub fn id(&self) -> uuid::Uuid {
        self.agent.id()
    }

    /// Model name used by this verifier.
    pub fn model_name(&self) -> &str {
        self.agent.model_name()
    }

    /// Verify a list of built entities against the source text.
    /// Returns the survivors (confirmed unchanged + corrected with
    /// adjustments + refinement marker), with rejects dropped.
    ///
    /// When `entities` is empty, returns immediately without an
    /// LLM call.
    #[tracing::instrument(
        target = TARGET,
        skip_all,
        fields(text_len = text.len(), entity_count = entities.len()),
    )]
    pub async fn verify(
        &self,
        text: &str,
        entities: Vec<Entity<Text>>,
    ) -> Result<Vec<Entity<Text>>> {
        if entities.is_empty() {
            return Ok(entities);
        }

        let prompt = NerVerifyPromptBuilder::new(text, &entities).build();
        let output: VerificationOutput = self
            .agent
            .prompt_structured_raw(&prompt)
            .await
            .map_err(crate::error::convert)?;

        let outcome = output.apply_to_text(entities);
        if outcome.dropped_oor > 0 {
            tracing::debug!(
                target: TARGET,
                dropped_oor = outcome.dropped_oor,
                "verifier returned out-of-range indices"
            );
        }
        Ok(outcome.survivors)
    }
}

// Verdict-application tests live with their subject:
// [`VerificationOutput::apply_to_text`] in
// `agent/base/verification/mod.rs`.
