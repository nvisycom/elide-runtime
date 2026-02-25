//! NER (Named Entity Recognition) agent for textual PII/entity detection.

mod output;
mod prompt;

pub use output::{RawEntities, RawEntity};

use rig::completion::CompletionModel;

use nvisy_core::Error;

use crate::backend::{DetectionConfig, UsageTracker};

use super::base::{BaseAgent, BaseAgentConfig};
use prompt::{NerPromptBuilder, NER_SYSTEM_PROMPT};

/// Agent for textual PII/entity detection using LLM + NER.
///
/// Wraps [`BaseAgent`] with NER-specific prompts and output types.
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

    /// Access the usage tracker.
    pub fn tracker(&self) -> &UsageTracker {
        self.base.tracker()
    }

    /// Detect entities in text using structured output with text-based fallback.
    #[tracing::instrument(skip_all, fields(text_len = text.len(), mode = "ner"))]
    pub async fn detect(
        &self,
        text: &str,
        config: &DetectionConfig,
    ) -> Result<Vec<RawEntity>, Error> {
        let prompt = NerPromptBuilder::new(config).build(text);
        let result: RawEntities = self
            .base
            .prompt_structured(&prompt, config.system_prompt.as_deref())
            .await?;
        Ok(result.entities)
    }
}
