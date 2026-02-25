//! Structured output types for redaction recommendations.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use nvisy_ontology::specification::TextRedactionMethod;

/// A single redaction recommendation from the LLM.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct RawRedaction {
    /// The original entity text that should be redacted.
    pub entity_value: String,
    /// Recommended redaction method.
    pub method: TextRedactionMethod,
    /// The suggested replacement text (e.g. `"[EMAIL]"`, `"***"`).
    pub replacement: String,
    /// Brief explanation of why this method was chosen.
    pub reasoning: Option<String>,
}

/// Top-level structured output wrapper from the redactor agent.
#[derive(Debug, Default, Deserialize, Serialize, JsonSchema)]
pub struct RedactorOutput {
    /// Recommended redactions for each entity.
    pub redactions: Vec<RawRedaction>,
}
