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
pub use input::ProposedEntity;
pub use output::{VerificationOutput, VerificationStatus, VerifiedEntity};
use prompt::{OCR_SYSTEM_PROMPT, OcrPromptBuilder};
use uuid::Uuid;

use super::{AgentConfig, AgentProvider, BaseAgent};
use crate::backend::UsageTracker;
use crate::error::Error;

/// VLM agent that verifies NER-detected entities against the original image.
///
/// # Workflow
///
/// 1. Caller passes raw image bytes and proposed entities to
///    [`verify`](Self::verify).
/// 2. The agent base64-encodes the image and builds a user prompt via
///    [`OcrPromptBuilder`] listing each entity with its index.
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
        skip_all,
        fields(image_bytes = image_data.len(), entity_count = entities.len(), agent = "ocr"),
    )]
    pub async fn verify(
        &self,
        image_data: &[u8],
        entities: &[ProposedEntity],
    ) -> Result<VerificationOutput, Error> {
        let image_b64 = STANDARD.encode(image_data);
        tracing::debug!(
            b64_len = image_b64.len(),
            proposed = entities.len(),
            "encoded image, building verification prompt"
        );

        let prompt = OcrPromptBuilder::new(entities).build(&image_b64);

        let output: VerificationOutput = self.base.prompt_structured(&prompt).await?;

        tracing::info!(changed = output.entities.len(), "ocr verification complete");

        Ok(output)
    }
}
