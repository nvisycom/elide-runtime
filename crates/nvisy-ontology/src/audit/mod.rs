//! Audit trail records for data protection events.
//!
//! An [`Audit`] entry records an immutable event in the data protection
//! pipeline, carrying structured [`Explanation`] metadata for compliance.

mod explanation;
mod retention;

pub use explanation::{Explainable, Explanation};
pub use retention::{RetentionPolicy, RetentionScope};

use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use uuid::Uuid;

use nvisy_core::path::ContentSource;

/// Types that emit audit records.
pub trait Auditable {
    /// Produce an audit record for this event.
    fn to_audit(&self) -> Audit;
}

/// Kind of auditable action recorded in an [`Audit`] entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "jsonschema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum AuditAction {
    /// A sensitive entity was detected.
    Detection,
    /// A redaction was applied to an entity.
    Redaction,
    /// A human review was performed on a redaction.
    Review,
}

/// An immutable audit record tracking a data protection event.
///
/// Audit entries are emitted by pipeline actions and form a tamper-evident
/// log of all detection, redaction, and policy decisions.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "jsonschema", derive(schemars::JsonSchema))]
pub struct Audit {
    /// Content source identity and lineage.
    #[serde(flatten)]
    pub source: ContentSource,
    /// The kind of event this audit entry records.
    pub action: AuditAction,
    /// UTC timestamp when the event occurred.
    #[cfg_attr(feature = "jsonschema", schemars(with = "String"))]
    pub timestamp: Timestamp,
    /// Identifier of the related entity, if applicable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entity_id: Option<Uuid>,
    /// Identifier of the related redaction, if applicable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub redaction_id: Option<Uuid>,
    /// Identifier of the policy that was evaluated, if applicable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub policy_id: Option<Uuid>,
    /// Identifier of the source blob or document.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_id: Option<Uuid>,
    /// Identifier of the pipeline run that produced this entry.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub run_id: Option<Uuid>,
    /// Human or service account that triggered the event.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actor: Option<String>,
    /// Structured explainability metadata.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub explanation: Option<Explanation>,
    /// Additional unstructured details about the event.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<Map<String, Value>>,
}

impl Audit {
    /// Create a new audit record for the given action, timestamped to now.
    pub fn new(action: AuditAction) -> Self {
        Self {
            source: ContentSource::new(),
            action,
            timestamp: Timestamp::now(),
            entity_id: None,
            redaction_id: None,
            policy_id: None,
            source_id: None,
            run_id: None,
            actor: None,
            explanation: None,
            details: None,
        }
    }

    /// Associate this audit entry with a detected entity.
    pub fn with_entity_id(mut self, id: Uuid) -> Self {
        self.entity_id = Some(id);
        self
    }

    /// Associate this audit entry with a redaction.
    pub fn with_redaction_id(mut self, id: Uuid) -> Self {
        self.redaction_id = Some(id);
        self
    }

    /// Associate this audit entry with a pipeline run.
    pub fn with_run_id(mut self, id: Uuid) -> Self {
        self.run_id = Some(id);
        self
    }

    /// Record the human or service account that triggered the event.
    pub fn with_actor(mut self, actor: impl Into<String>) -> Self {
        self.actor = Some(actor.into());
        self
    }

    /// Attach additional unstructured details to this audit entry.
    pub fn with_details(mut self, details: Map<String, Value>) -> Self {
        self.details = Some(details);
        self
    }
}
