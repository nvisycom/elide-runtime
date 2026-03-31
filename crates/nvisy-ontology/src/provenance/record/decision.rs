//! Pipeline-facing redaction decision.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::entity::ContentSource;
use crate::policy::Strategy;

/// A pipeline-facing decision recording how a specific entity should be
/// redacted.
///
/// Carries the strategy and entity identity. The actual replacement
/// text or codec instruction is computed at application time by the
/// executor, not stored here. This keeps the decision modality-agnostic:
/// the same type works for text, image, and audio entities.
///
/// Does **not** retain the original sensitive value: that lives in
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
    /// Redaction strategy to apply.
    pub spec: Strategy,
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
    pub fn new(entity_id: Uuid, spec: Strategy, confidence: f64) -> Self {
        Self {
            source: ContentSource::new(),
            entity_id,
            spec,
            confidence,
            policy_rule_id: None,
            applied: false,
        }
    }

    /// The unique identifier for this decision.
    pub fn id(&self) -> Uuid {
        self.source.as_uuid()
    }

    /// Associate this decision with the policy rule that triggered it.
    pub fn with_policy_rule_id(mut self, id: Uuid) -> Self {
        self.policy_rule_id = Some(id);
        self
    }
}
