//! NER-specific prompt construction.

use crate::backend::DetectionConfig;
use crate::bridge::PromptBuilder;

use super::KnownNerEntity;

/// Builds user prompts for NER entity detection.
pub(crate) struct NerPromptBuilder<'a> {
    inner: PromptBuilder<'a>,
    known_entities: &'a [KnownNerEntity],
}

impl<'a> NerPromptBuilder<'a> {
    /// Create a prompt builder from a [`DetectionConfig`].
    pub fn new(config: &'a DetectionConfig, known_entities: &'a [KnownNerEntity]) -> Self {
        Self {
            inner: PromptBuilder::new(config),
            known_entities,
        }
    }

    /// Build the user prompt for the given text.
    pub fn build(&self, text: &str) -> String {
        let mut prompt = self.inner.build(text);

        if !self.known_entities.is_empty() {
            prompt.push_str("\n\nPreviously identified entities (reuse their entity_id for coreferent mentions):\n");
            for e in self.known_entities {
                let type_str = match &e.entity_type {
                    Some(t) => t.to_string(),
                    None => "unknown".to_string(),
                };
                let values = e.values.iter().map(|v| format!("\"{v}\"")).collect::<Vec<_>>().join(", ");
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
