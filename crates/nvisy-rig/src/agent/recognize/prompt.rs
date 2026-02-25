//! NER-specific prompt construction.

use crate::backend::DetectionConfig;
use crate::bridge::PromptBuilder;

/// Builds user prompts for NER entity detection.
pub(crate) struct NerPromptBuilder<'a> {
    inner: PromptBuilder<'a>,
}

impl<'a> NerPromptBuilder<'a> {
    /// Create a prompt builder from a [`DetectionConfig`].
    pub fn new(config: &'a DetectionConfig) -> Self {
        Self {
            inner: PromptBuilder::new(config),
        }
    }

    /// Build the user prompt for the given text.
    pub fn build(&self, text: &str) -> String {
        self.inner.build(text)
    }
}

/// Default system prompt for NER detection.
pub(super) const NER_SYSTEM_PROMPT: &str = "\
You are a precise named-entity recognition system. \
Identify personally identifiable information (PII), protected health information (PHI), \
financial data, and credentials in the provided text. \
Return results as a JSON array of objects with keys: \
category, entity_type, value, confidence, start_offset, end_offset. \
If no entities are found, return an empty array [].";
