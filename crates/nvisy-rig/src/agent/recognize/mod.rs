//! Named Entity Recognition (NER) agent for textual PII/entity detection.
//!
//! [`NerAgent`] wraps a [`BaseAgent`](super::BaseAgent) with NER-specific
//! prompts. It is a pure LLM agent (no tools) that analyses text and
//! returns structured entity detections with byte offsets.

mod output;
mod prompt;

pub use output::{RawEntities, RawEntity};

use rig::completion::CompletionModel;

use nvisy_core::Error;

use crate::backend::{DetectionConfig, UsageTracker};

use super::base::{BaseAgent, BaseAgentConfig};
use prompt::{NerPromptBuilder, NER_SYSTEM_PROMPT};

/// Agent for textual PII/entity detection using LLM-based NER.
///
/// # Workflow
///
/// 1. Caller passes text and a [`DetectionConfig`] to
///    [`detect`](Self::detect).
/// 2. The agent builds a user prompt via [`NerPromptBuilder`] that
///    specifies entity types and confidence thresholds.
/// 3. Structured output is parsed into `Vec<RawEntity>`.
pub struct NerAgent<M: CompletionModel> {
    base: BaseAgent<M>,
}

impl<M: CompletionModel> NerAgent<M> {
    /// Create a new NER agent with the given model and config.
    pub fn new(model: M, config: BaseAgentConfig) -> Self {
        let base = BaseAgent::builder(model, config)
            .preamble(NER_SYSTEM_PROMPT)
            .build();
        Self { base }
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
    ) -> Result<Vec<RawEntity>, Error> {
        let prompt = NerPromptBuilder::new(config).build(text);

        tracing::debug!(
            prompt_len = prompt.len(),
            entity_kinds = config.entity_kinds.len(),
            "built ner prompt"
        );

        let result: RawEntities = self
            .base
            .prompt_structured(&prompt, config.system_prompt.as_deref())
            .await?;

        tracing::info!(
            entity_count = result.entities.len(),
            "ner detection complete"
        );

        Ok(result.entities)
    }
}
