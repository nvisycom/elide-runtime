//! [`EntityVerifier`]: LLM-driven verification of proposed entities
//! against an image.
//!
//! Given a list of [`ProposedEntity`] values (typically produced
//! upstream by OCR + NER) and the original image bytes, prompts a
//! vision-capable LLM to confirm, correct, or reject each one.
//! Returns a [`VerificationOutput`] containing only changed
//! entities; confirmed entities are implicitly absent.
//!
//! This is the pure LLM half of what used to be `OcrAgent`. The
//! OCR-provider orchestration (running an OCR engine, building
//! `ProposedEntity` from its output, calling the verifier, merging
//! the verdict back) now lives in
//! `nvisy_engine::operation::extraction::vision`.

mod input;
mod output;
mod prompt;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use bytes::Bytes;
use nvisy_core::Result;
use uuid::Uuid;

pub use self::input::{ProposedEntity, VerificationCandidate};
pub use self::output::{VerificationOutput, VerificationStatus, VerifiedEntity};
use self::prompt::{ENTITY_VERIFIER_SYSTEM_PROMPT, EntityVerifierPromptBuilder};
use crate::agent::base::UsageTracker;
use crate::agent::{AgentConfig, AgentProvider, BaseAgent};

const TARGET: &str = "nvisy_rig::agent::entity_verifier";

/// LLM-driven entity verifier. Wraps an internal `BaseAgent` with
/// the entity-verification system prompt baked in.
pub struct EntityVerifier {
    base: BaseAgent,
}

impl EntityVerifier {
    /// Construct a verifier from an LLM provider + agent config.
    ///
    /// The config's preamble defaults to the built-in
    /// entity-verification system prompt when unset.
    pub fn new(provider: &AgentProvider, mut config: AgentConfig) -> Result<Self> {
        config
            .preamble
            .get_or_insert_with(|| ENTITY_VERIFIER_SYSTEM_PROMPT.into());
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

        let prompt = EntityVerifierPromptBuilder::new(entities).build(&image_b64);
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

    /// Access the usage tracker for the underlying agent.
    pub fn tracker(&self) -> &UsageTracker {
        self.base.tracker()
    }

    /// Unique identifier for the underlying agent instance.
    pub fn id(&self) -> Uuid {
        self.base.id()
    }
}
