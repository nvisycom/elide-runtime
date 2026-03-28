//! OCR verification agent for reviewing NER-detected entities against images.
//!
//! [`OcrAgent`] wraps a [`BaseAgent`](crate::backend::BaseAgent) with
//! verification-specific prompts. It is a pure LLM agent (no tools) that
//! reviews proposed entities against the original image and returns only
//! those that need correction or rejection.

mod input;
mod output;
mod prompt;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use nvisy_ontology::entity::Entity;
use uuid::Uuid;

pub use self::input::ProposedEntity;
pub use self::output::{VerificationOutput, VerificationStatus, VerifiedEntity};
use self::prompt::{OCR_SYSTEM_PROMPT, OcrPromptBuilder};
use super::base::UsageTracker;
use super::{AgentConfig, AgentProvider, BaseAgent};
use crate::error::Error;

const TARGET: &str = "nvisy_provider::agent::ocr";

/// VLM agent that verifies NER-detected entities against the original image.
///
/// # Workflow
///
/// 1. Caller passes raw image bytes and proposed entities to
///    [`verify`](Self::verify).
/// 2. The agent base64-encodes the image and builds a user prompt via
///    `OcrPromptBuilder` listing each entity with its index.
/// 3. The VLM reviews entities against the image and returns only those
///    needing correction or rejection.
/// 4. Structured output is parsed into [`VerificationOutput`].
pub struct OcrAgent {
    base: BaseAgent,
}

impl OcrAgent {
    /// Create a new OCR verification agent.
    pub fn new(provider: &AgentProvider, mut config: AgentConfig) -> Result<Self, Error> {
        config
            .preamble
            .get_or_insert_with(|| OCR_SYSTEM_PROMPT.into());
        let base = BaseAgent::builder(provider, config).build()?;
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

    /// Verify proposed entities against the original image.
    ///
    /// Returns only entities that were corrected or rejected. Entities
    /// absent from the output are implicitly confirmed.
    #[tracing::instrument(
        target = "nvisy_provider::agent::ocr",
        skip_all,
        fields(image_bytes = image_data.len(), entity_count = entities.len()),
    )]
    pub async fn verify(
        &self,
        image_data: &[u8],
        entities: &[ProposedEntity],
    ) -> Result<VerificationOutput, Error> {
        let image_b64 = STANDARD.encode(image_data);
        tracing::debug!(
            target: TARGET,
            b64_len = image_b64.len(),
            proposed = entities.len(),
            "encoded image, building verification prompt"
        );

        let prompt = OcrPromptBuilder::new(entities).build(&image_b64);

        let output: VerificationOutput = self.base.prompt_structured(&prompt).await?;

        tracing::info!(target: TARGET, changed = output.entities.len(), "ocr verification complete");

        Ok(output)
    }

    /// Verify entities against the original image, returning the merged result.
    ///
    /// Converts each [`Entity`] into a [`ProposedEntity`], calls
    /// [`verify`](Self::verify), then merges verdicts back: confirmed
    /// entities pass through unchanged, corrected entities are updated,
    /// and rejected entities are dropped.
    #[tracing::instrument(
        target = "nvisy_provider::agent::ocr",
        skip_all,
        fields(image_bytes = image_data.len(), entity_count = entities.len()),
    )]
    pub async fn verify_entities(
        &self,
        image_data: &[u8],
        entities: Vec<Entity>,
    ) -> Result<Vec<Entity>, Error> {
        if entities.is_empty() {
            return Ok(Vec::new());
        }

        let proposed: Vec<ProposedEntity> = entities
            .iter()
            .enumerate()
            .map(|(i, e)| ProposedEntity::from_entity(i, e))
            .collect();

        let output = self.verify(image_data, &proposed).await?;

        Ok(output.merge(entities))
    }
}
