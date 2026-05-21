//! NER-specific prompt construction.

use super::KnownNerEntity;
use crate::agent::{ALL_TYPES_HINT, DetectionConfig};

/// Instruction prefix for the user prompt.
const DETECT_PREFIX: &str = "Detect entities of types";

/// Suffix describing the expected response format.
const RESPONSE_FORMAT: &str = "\
Return a JSON array of objects with keys: \
entity_id, category, entity_type, value, confidence, context.";

/// Builds user prompts for NER entity detection.
pub(crate) struct NerPromptBuilder<'a> {
    config: &'a DetectionConfig,
    known_entities: &'a [KnownNerEntity],
}

impl<'a> NerPromptBuilder<'a> {
    /// Create a prompt builder from a [`DetectionConfig`].
    pub fn new(config: &'a DetectionConfig, known_entities: &'a [KnownNerEntity]) -> Self {
        Self {
            config,
            known_entities,
        }
    }

    /// Build the user prompt for the given text.
    pub fn build(&self, text: &str) -> String {
        let types_hint = if self.config.entity_kinds.is_empty() {
            ALL_TYPES_HINT.to_string()
        } else {
            self.config
                .entity_kinds
                .iter()
                .map(|e| e.to_string())
                .collect::<Vec<_>>()
                .join(", ")
        };

        let threshold_clause = match self.config.confidence_threshold {
            Some(t) => format!(" with minimum confidence {t:.2}"),
            None => String::new(),
        };
        let mut prompt = format!(
            "{DETECT_PREFIX} [{types_hint}]{threshold_clause} in the following text. \
             {RESPONSE_FORMAT}\n\n---\n{text}\n---",
        );

        if !self.known_entities.is_empty() {
            prompt.push_str("\n\nPreviously identified entities (reuse their entity_id for coreferent mentions):\n");
            for e in self.known_entities {
                let type_str = match &e.entity_type {
                    Some(t) => t.to_string(),
                    None => "unknown".to_string(),
                };
                let values = e
                    .values
                    .iter()
                    .map(|v| format!("\"{v}\""))
                    .collect::<Vec<_>>()
                    .join(", ");
                prompt.push_str(&format!(
                    "- entity_id={}, type={}, values=[{}]",
                    e.entity_id, type_str, values,
                ));
                if !e.descriptions.is_empty() {
                    let descs = e.descriptions.join("; ");
                    prompt.push_str(&format!(", description=\"{descs}\""));
                }
                prompt.push('\n');
            }
        }

        prompt
    }
}

/// Default system prompt for NER detection.
pub(super) const NER_SYSTEM_PROMPT: &str = "\
You are a precise named-entity recognition system. \
Identify personally identifiable information (PII), protected health information (PHI), \
financial data, and credentials in the provided text. \
Return results as a JSON object with an \"entities\" key containing an array of objects with keys: \
entity_id, category (optional), entity_type (optional), value, confidence (optional), \
context (optional), description (optional). \
Assign a stable entity_id (e.g. \"person_1\", \"org_1\") to each unique real-world entity. \
All mentions of the same entity must share the same entity_id. \
When previously identified entities are provided, reuse their entity_id for any coreferent mentions. \
The \"context\" field should be a short surrounding snippet of text that uniquely locates this \
mention within the input. Include enough words before and after the value so that the context \
string appears exactly once in the input text. This is especially important when the same value \
(e.g. \"he\") appears multiple times. \
The \"description\" field should be a brief description of the real-world entity \
(e.g. \"CEO of Acme Corp\", \"patient's home address\"). Provide it for the first mention \
of each entity or when additional context becomes available. \
If no entities are found, return {\"entities\": []}.";

#[cfg(test)]
mod tests {
    use nvisy_ontology::entity::EntityKind;

    use super::*;

    #[test]
    fn builds_prompt_with_entity_kinds() {
        let config = DetectionConfig {
            entity_kinds: vec![EntityKind::PersonName, EntityKind::GovernmentId],
            confidence_threshold: Some(0.7),
            system_prompt: None,
        };
        let prompt = NerPromptBuilder::new(&config, &[]).build("Hello world");
        assert!(prompt.contains("person_name, government_id"));
        assert!(prompt.contains("0.70"));
        assert!(prompt.contains("Hello world"));
    }

    #[test]
    fn builds_prompt_without_entity_kinds() {
        let config = DetectionConfig {
            entity_kinds: vec![],
            confidence_threshold: Some(0.5),
            system_prompt: None,
        };
        let prompt = NerPromptBuilder::new(&config, &[]).build("test");
        assert!(prompt.contains("all entity types"));
    }

    #[test]
    fn builds_prompt_with_known_entities() {
        let config = DetectionConfig {
            entity_kinds: vec![],
            confidence_threshold: Some(0.8),
            system_prompt: None,
        };
        let known = vec![KnownNerEntity {
            entity_id: "person_1".to_string(),
            entity_type: Some(EntityKind::PersonName),
            values: vec!["John".to_string()],
            descriptions: vec!["CEO of Acme".to_string()],
        }];
        let prompt = NerPromptBuilder::new(&config, &known).build("some text");
        assert!(prompt.contains("entity_id=person_1"));
        assert!(prompt.contains("CEO of Acme"));
    }
}
