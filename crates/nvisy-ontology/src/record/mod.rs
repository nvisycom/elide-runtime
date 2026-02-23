//! Redaction decision records.

mod review;

pub use review::{ReviewDecision, ReviewStatus};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use nvisy_core::path::ContentSource;

use crate::spec::RedactionInput;

/// A redaction decision recording how a specific entity was (or will be) redacted.
///
/// Each `Redaction` is linked to exactly one entity via `entity_id`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[derive(JsonSchema)]
pub struct Redaction {
    /// Content source identity and lineage.
    #[serde(flatten)]
    pub source: ContentSource,
    /// Identifier of the entity being redacted.
    pub entity_id: Uuid,
    /// Redaction specification recording the method used (provenance for audit).
    pub spec: RedactionInput,
    /// Resolved replacement string (empty for Remove, unused for image/audio).
    pub replacement: String,
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
        spec: RedactionInput,
        replacement: impl Into<String>,
        original_value: impl Into<String>,
        confidence: f64,
    ) -> Self {
        Self {
            source: ContentSource::new(),
            entity_id,
            spec,
            replacement: replacement.into(),
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
