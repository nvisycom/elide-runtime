//! Prompt construction for LLM entity detection.

use crate::backend::LlmConfig;

/// Builds user prompts for entity detection requests.
pub struct PromptBuilder<'a> {
    entity_types: &'a [String],
    confidence_threshold: f64,
}

impl<'a> PromptBuilder<'a> {
    /// Create a prompt builder from an [`LlmConfig`].
    pub fn new(config: &'a LlmConfig) -> Self {
        Self {
            entity_types: &config.entity_types,
            confidence_threshold: config.confidence_threshold,
        }
    }

    /// Build the user prompt for the given text.
    pub fn build(&self, text: &str) -> String {
        let types_hint = if self.entity_types.is_empty() {
            "all entity types".to_string()
        } else {
            self.entity_types.join(", ")
        };

        format!(
            "Detect entities of types [{types_hint}] with minimum confidence \
             {threshold:.2} in the following text. Return a JSON array of objects \
             with keys: category, entity_type, value, confidence, start_offset, \
             end_offset.\n\n---\n{text}\n---",
            types_hint = types_hint,
            threshold = self.confidence_threshold,
            text = text,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_prompt_with_entity_types() {
        let config = LlmConfig {
            entity_types: vec!["PERSON".into(), "SSN".into()],
            confidence_threshold: 0.7,
            system_prompt: None,
        };
        let prompt = PromptBuilder::new(&config).build("Hello world");
        assert!(prompt.contains("PERSON, SSN"));
        assert!(prompt.contains("0.70"));
        assert!(prompt.contains("Hello world"));
    }

    #[test]
    fn builds_prompt_without_entity_types() {
        let config = LlmConfig {
            entity_types: vec![],
            confidence_threshold: 0.5,
            system_prompt: None,
        };
        let prompt = PromptBuilder::new(&config).build("test");
        assert!(prompt.contains("all entity types"));
    }
}
