//! Audit-facing redaction record.

use nvisy_core::content::ContentSource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::review::ReviewDecision;

/// An audit-facing record of a redaction that was (or will be) applied.
///
/// `RedactionRecord` retains the original sensitive value for audit purposes
/// and tracks versioning and human review. It does **not** carry the redaction
/// spec or replacement — those live in [`RedactionDecision`](super::RedactionDecision).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RedactionRecord {
    /// Content source identity and lineage.
    pub source: ContentSource,
    /// Identifier of the entity being redacted.
    pub entity_id: Uuid,
    /// The original sensitive value, retained for audit purposes.
    pub original_value: String,
    /// Detection confidence that led to this redaction.
    pub confidence: f64,
    /// Identifier of the policy rule that triggered this redaction.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub policy_rule_id: Option<Uuid>,
    /// Version of this redaction record (starts at 1, incremented on modification).
    pub version: u32,
    /// Human review decision, if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub review: Option<ReviewDecision>,
}

impl RedactionRecord {
    /// Create a new audit record for the given entity.
    pub fn new(entity_id: Uuid, original_value: impl Into<String>, confidence: f64) -> Self {
        Self {
            source: ContentSource::new(),
            entity_id,
            original_value: original_value.into(),
            confidence,
            policy_rule_id: None,
            version: 1,
            review: None,
        }
    }

    /// The unique identifier for this record (delegates to `source.as_uuid()`).
    pub fn id(&self) -> Uuid {
        self.source.as_uuid()
    }

    /// Associate this record with the policy rule that triggered it.
    pub fn with_policy_rule_id(mut self, id: Uuid) -> Self {
        self.policy_rule_id = Some(id);
        self
    }
}
