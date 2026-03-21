//! Pipeline-facing redaction decision.

use nvisy_core::content::ContentSource;
use nvisy_ontology::policy::Strategy;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A pipeline-facing decision recording how a specific entity should be redacted.
///
/// `RedactionDecision` carries the information needed by the redaction operation
/// to apply a redaction: the spec, replacement text, and whether it has been
/// applied. It does **not** retain the original sensitive value — that lives in
/// [`RedactionRecord`].
///
/// [`RedactionRecord`]: super::RedactionRecord
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RedactionDecision {
    /// Content source identity and lineage.
    pub source: ContentSource,
    /// Identifier of the entity being redacted.
    pub entity_id: Uuid,
    /// Redaction strategy recording the method used.
    pub spec: Strategy,
    /// Resolved replacement string (empty for Remove, unused for image/audio).
    pub replacement: String,
    /// Detection confidence that led to this redaction.
    pub confidence: f64,
    /// Identifier of the policy rule that triggered this redaction.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub policy_rule_id: Option<Uuid>,
    /// Whether the redaction has been applied to the output content.
    pub applied: bool,
}

impl RedactionDecision {
    /// Create a new pending redaction decision for the given entity.
    pub fn new(
        entity_id: Uuid,
        spec: Strategy,
        replacement: impl Into<String>,
        confidence: f64,
    ) -> Self {
        Self {
            source: ContentSource::new(),
            entity_id,
            spec,
            replacement: replacement.into(),
            confidence,
            policy_rule_id: None,
            applied: false,
        }
    }

    /// The unique identifier for this decision (delegates to `source.as_uuid()`).
    pub fn id(&self) -> Uuid {
        self.source.as_uuid()
    }

    /// Associate this decision with the policy rule that triggered it.
    pub fn with_policy_rule_id(mut self, id: Uuid) -> Self {
        self.policy_rule_id = Some(id);
        self
    }
}
