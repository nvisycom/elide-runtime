//! Named Entity Recognition (NER) agent for textual PII/entity detection.
//!
//! [`NerAgent`] wraps a [`BaseAgent`](crate::backend::BaseAgent) with
//! NER-specific prompts. It is a pure LLM agent (no tools) that analyses
//! text and returns structured entity detections with byte offsets.

mod output;
mod prompt;

pub use output::{NerEntities, NerEntity};

use uuid::Uuid;

use crate::backend::{BaseAgent, BaseAgentConfig, DetectionConfig, Provider, UsageTracker};
use crate::error::Error;
use prompt::{NER_SYSTEM_PROMPT, NerPromptBuilder};

/// Agent for textual PII/entity detection using LLM-based NER.
///
/// # Workflow
///
/// 1. Caller passes text and a [`DetectionConfig`] to
///    [`detect`](Self::detect).
/// 2. The agent builds a user prompt via [`NerPromptBuilder`] that
///    specifies entity types and confidence thresholds.
/// 3. Structured output is parsed into `Vec<NerEntity>`.
pub struct NerAgent {
    base: BaseAgent,
}

impl NerAgent {
    /// Create a new NER agent.
    pub fn new(provider: &Provider, config: BaseAgentConfig) -> Result<Self, Error> {
        let base = BaseAgent::builder(provider, config)
            .preamble(NER_SYSTEM_PROMPT)
            .build()?;
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

    /// Detect entities in text using structured output with text-based fallback.
    #[tracing::instrument(
        skip_all,
        fields(text_len = text.len(), agent = "ner"),
    )]
    pub async fn detect(
        &self,
        text: &str,
        config: &DetectionConfig,
    ) -> Result<Vec<NerEntity>, Error> {
        let prompt = NerPromptBuilder::new(config).build(text);

        tracing::debug!(
            prompt_len = prompt.len(),
            entity_kinds = config.entity_kinds.len(),
            "built ner prompt"
        );

        let result: NerEntities = self.base.prompt_structured(&prompt).await?;

        tracing::info!(
            entity_count = result.entities.len(),
            "ner detection complete"
        );

        Ok(result.entities)
    }
}
