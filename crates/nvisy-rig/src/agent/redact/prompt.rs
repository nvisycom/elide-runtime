//! Redactor-specific prompt construction.
//!
//! [`RedactorPromptBuilder`] constructs the user prompt that presents
//! detected entities and surrounding text to the LLM for redaction
//! method selection.

use nvisy_core::Error;
use nvisy_ontology::specification::RedactorInput;

/// Builds user prompts for redaction recommendations.
///
/// Serialises the entity list as JSON and wraps the source text in
/// delimiters so the LLM has full context for sensitivity-aware decisions.
pub(crate) struct RedactorPromptBuilder;

impl RedactorPromptBuilder {
    /// Build the user prompt for the given text and entity list.
    pub fn build(text: &str, entities: &[RedactorInput]) -> Result<String, Error> {
        let entities_json = serde_json::to_string_pretty(entities).map_err(|e| {
            Error::runtime(
                format!("failed to serialize entities for redactor: {e}"),
                "rig",
                false,
            )
        })?;

        Ok(format!(
            "Recommend redaction methods for the following entities found in the \
             text below.\n\n\
             Entities:\n{entities_json}\n\n\
             ---\n{text}\n---"
        ))
    }
}

/// Default system prompt for the redactor agent.
pub(super) const REDACTOR_SYSTEM_PROMPT: &str = "\
You are a context-aware redaction system. Given a text and a list of detected entities, \
recommend the most appropriate redaction method for each entity.\n\
\n\
Available redaction methods:\n\
- \"mask\": Replace with a fixed mask (e.g. \"***\", \"[REDACTED]\"). Use for highly sensitive data \
  where the original value must not be recoverable.\n\
- \"replace\": Replace with a type-appropriate placeholder (e.g. \"[EMAIL]\", \"[SSN]\"). Use when \
  the entity type should remain visible but the value hidden.\n\
- \"hash\": Replace with a deterministic hash. Use when linkability across documents is needed \
  without exposing the original value.\n\
- \"synthesize\": Replace with a realistic but fake value (e.g. a fake name, fake address). Use \
  when preserving data format and statistical properties matters.\n\
- \"pseudonymize\": Replace with a consistent pseudonym. Use when the same entity should map to \
  the same pseudonym across a document or dataset.\n\
- \"remove\": Delete the entity entirely. Use for data that adds no analytical value.\n\
\n\
For each entity, consider:\n\
- Sensitivity level (credentials > government IDs > names)\n\
- Context (medical records need stricter redaction than marketing copy)\n\
- Downstream utility (will analysts need to correlate redacted values?)\n\
\n\
Return a JSON object with a \"redactions\" array. Each element must have:\n\
- \"entity_value\": the original entity text\n\
- \"method\": one of the methods above\n\
- \"replacement\": the suggested replacement text\n\
- \"reasoning\": brief explanation of why this method was chosen (optional)\n\
\n\
If no redactions are needed, return {\"redactions\": []}.";
