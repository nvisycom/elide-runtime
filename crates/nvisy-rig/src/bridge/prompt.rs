//! Prompt construction for LLM entity detection.

use std::fmt::Display;

use nvisy_ontology::entity::EntityKind;

use crate::backend::{DetectionConfig, ALL_TYPES_HINT};

/// Instruction prefix for the user prompt.
const DETECT_PREFIX: &str = "Detect entities of types";

/// Suffix describing the expected response format.
const RESPONSE_FORMAT: &str = "\
Return a JSON array of objects with keys: \
category, entity_type, value, confidence, start_offset, end_offset.";

/// Builds user prompts for entity detection requests.
pub struct PromptBuilder<'a> {
    entity_kinds: &'a [EntityKind],
    confidence_threshold: f64,
}

impl<'a> PromptBuilder<'a> {
    /// Create a prompt builder from a [`DetectionConfig`].
    pub fn new(config: &'a DetectionConfig) -> Self {
        Self {
            entity_kinds: &config.entity_kinds,
            confidence_threshold: config.confidence_threshold,
        }
    }

    /// Build the user prompt for the given text.
    pub fn build(&self, text: &str) -> String {
        self.build_for(self.entity_kinds, text)
    }

    /// Build a prompt using an arbitrary slice of displayable entity labels.
    ///
    /// This allows callers to pass any `Vec<E>` where `E: Display` — for
    /// example custom string labels or [`EntityKind`] variants.
    pub fn build_for<E: Display>(&self, entity_types: &[E], text: &str) -> String {
        let types_hint = if entity_types.is_empty() {
            ALL_TYPES_HINT.to_string()
        } else {
            entity_types.iter().map(|e| e.to_string()).collect::<Vec<_>>().join(", ")
        };

        format!(
            "{DETECT_PREFIX} [{types_hint}] with minimum confidence \
             {threshold:.2} in the following text. {RESPONSE_FORMAT}\n\n---\n{text}\n---",
            threshold = self.confidence_threshold,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_prompt_with_entity_kinds() {
        let config = DetectionConfig {
            entity_kinds: vec![EntityKind::PersonName, EntityKind::GovernmentId],
            confidence_threshold: 0.7,
            system_prompt: None,
        };
        let prompt = PromptBuilder::new(&config).build("Hello world");
        assert!(prompt.contains("person_name, government_id"));
        assert!(prompt.contains("0.70"));
        assert!(prompt.contains("Hello world"));
    }

    #[test]
    fn builds_prompt_without_entity_kinds() {
        let config = DetectionConfig {
            entity_kinds: vec![],
            confidence_threshold: 0.5,
            system_prompt: None,
        };
        let prompt = PromptBuilder::new(&config).build("test");
        assert!(prompt.contains("all entity types"));
    }

    #[test]
    fn build_for_with_string_labels() {
        let config = DetectionConfig {
            entity_kinds: vec![],
            confidence_threshold: 0.8,
            system_prompt: None,
        };
        let builder = PromptBuilder::new(&config);
        let labels = vec!["PERSON", "SSN"];
        let prompt = builder.build_for(&labels, "some text");
        assert!(prompt.contains("PERSON, SSN"));
        assert!(prompt.contains("0.80"));
    }
}
