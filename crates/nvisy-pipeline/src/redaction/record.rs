//! Redaction decision records.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use nvisy_codec::transform::RedactionOutput;
use nvisy_core::path::ContentSource;

use super::review::ReviewDecision;

/// A redaction decision recording how a specific entity was (or will be) redacted.
///
/// Each `Redaction` is linked to exactly one entity via `entity_id`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[derive(schemars::JsonSchema)]
pub struct Redaction {
    /// Content source identity and lineage.
    #[serde(flatten)]
    pub source: ContentSource,
    /// Identifier of the entity being redacted.
    pub entity_id: Uuid,
    /// Redaction output recording the method used and its result data.
    pub output: RedactionOutput,
    /// The original sensitive value, retained for audit purposes.
    pub original_value: String,
    /// Detection confidence that led to this redaction.
    pub confidence: f64,
    /// Identifier of the policy rule that triggered this redaction.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub policy_rule_id: Option<Uuid>,
    /// Whether the redaction has been applied to the output content.
    pub applied: bool,
    /// Version of this redaction record (starts at 1, incremented on modification).
    pub version: u32,
    /// Human review decision, if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub review: Option<ReviewDecision>,
}

impl Redaction {
    /// Create a new pending redaction for the given entity.
    pub fn new(
        entity_id: Uuid,
        output: impl Into<RedactionOutput>,
        original_value: impl Into<String>,
        confidence: f64,
    ) -> Self {
        Self {
            source: ContentSource::new(),
            entity_id,
            output: output.into(),
            original_value: original_value.into(),
            confidence,
            policy_rule_id: None,
            applied: false,
            version: 1,
            review: None,
        }
    }

    /// Associate this redaction with the policy rule that triggered it.
    pub fn with_policy_rule_id(mut self, id: Uuid) -> Self {
        self.policy_rule_id = Some(id);
        self
    }
}
