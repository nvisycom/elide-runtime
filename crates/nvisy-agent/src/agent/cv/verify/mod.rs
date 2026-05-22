//! [`CvVerifyAgent`]: LLM-driven verification of proposed entities
//! against an image.
//!
//! Given a list of [`ProposedEntity`] values (typically produced
//! upstream by OCR + NER) and the original image bytes, prompts a
//! vision-capable LLM to confirm, correct, or reject each one.
//! Returns a [`VerificationOutput`] containing only changed
//! entities; confirmed entities are implicitly absent.

mod input;
mod output;
mod prompt;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use bytes::Bytes;
use nvisy_core::Result;
use nvisy_ontology::entity::Entity;
use uuid::Uuid;

pub use self::input::{ProposedEntity, VerificationCandidate};
use self::prompt::{CV_VERIFIER_SYSTEM_PROMPT, CvVerifyAgentPromptBuilder};
use crate::agent::base::{BaseAgent, UsageTracker, VerificationOutput};
use crate::agent::{AgentConfig, AgentProvider};

const TARGET: &str = "nvisy_agent::agent::cv_verify_agent";

/// LLM-driven entity verifier. Wraps an internal `BaseAgent` with
/// the entity-verification system prompt baked in.
pub struct CvVerifyAgent {
    base: BaseAgent,
}

impl CvVerifyAgent {
    /// Construct a verifier from an LLM provider + agent config.
    ///
    /// The config's preamble defaults to the built-in
    /// entity-verification system prompt when unset.
    pub fn new(provider: &AgentProvider, mut config: AgentConfig) -> Result<Self> {
        config
            .preamble
            .get_or_insert_with(|| CV_VERIFIER_SYSTEM_PROMPT.into());
        let base = BaseAgent::builder(provider, config)
            .build()
            .map_err(crate::error::convert)?;
        Ok(Self { base })
    }

    /// Verify proposed entities against the original image.
    ///
    /// Returns only entities that were corrected or rejected.
    /// Entities absent from the output are implicitly confirmed.
    #[tracing::instrument(
        target = TARGET,
        skip_all,
        fields(image_bytes = image_data.len(), entity_count = entities.len()),
    )]
    pub async fn verify(
        &self,
        image_data: &Bytes,
        entities: &[ProposedEntity],
    ) -> Result<VerificationOutput> {
        let image_b64 = STANDARD.encode(image_data);
        tracing::debug!(
            target: TARGET,
            b64_len = image_b64.len(),
            proposed = entities.len(),
            "encoded image, building verification prompt"
        );

        let prompt = CvVerifyAgentPromptBuilder::new(entities).build(&image_b64);
        let output: VerificationOutput = self
            .base
            .prompt_structured_raw(&prompt)
            .await
            .map_err(crate::error::convert)?;

        tracing::info!(
            target: TARGET,
            changed = output.entities.len(),
            "entity verification complete"
        );

        Ok(output)
    }

    /// Verify proposed entities and merge verdicts back into the
    /// original entity list.
    ///
    /// Confirmed entities pass through unchanged, corrected
    /// entities are updated with the verifier's bbox / value /
    /// type / category, and rejected entities are dropped.
    pub async fn verify_entities(
        &self,
        image_data: &Bytes,
        candidates: Vec<VerificationCandidate>,
    ) -> Result<Vec<Entity>> {
        if candidates.is_empty() {
            return Ok(Vec::new());
        }
        let proposed: Vec<ProposedEntity> = candidates
            .iter()
            .enumerate()
            .map(|(i, c)| ProposedEntity::from_entity(i, &c.entity, &c.value))
            .collect();
        let entities_only: Vec<Entity> = candidates.into_iter().map(|c| c.entity).collect();
        let output = self.verify(image_data, &proposed).await?;
        Ok(output::merge(output, entities_only))
    }

    /// Access the usage tracker for the underlying agent.
    pub fn tracker(&self) -> &UsageTracker {
        self.base.tracker()
    }

    /// Unique identifier for the underlying agent instance.
    pub fn id(&self) -> Uuid {
        self.base.id()
    }

    /// Configured model name.
    pub fn model_name(&self) -> &str {
        self.base.model_name()
    }
}
