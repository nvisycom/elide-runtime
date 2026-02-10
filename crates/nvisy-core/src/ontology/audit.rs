//! Audit trail records for data protection events.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use crate::datatypes::Data;
use crate::datatypes::Metadata;

/// Kind of auditable action recorded in an [`Audit`] entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum AuditAction {
    /// A sensitive entity was detected.
    Detection,
    /// A redaction was applied to an entity.
    Redaction,
    /// A policy was evaluated against detected entities.
    PolicyEval,
    /// A blob or document was accessed.
    Access,
    /// Processed content was exported to an external system.
    Export,
}

/// An immutable audit record tracking a data protection event.
///
/// Audit entries are emitted by pipeline actions and form a tamper-evident
/// log of all detection, redaction, and policy decisions.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct Audit {
    /// Common data-item fields (id, parent_id, metadata).
    #[serde(flatten)]
    pub data: Data,
    /// The kind of event this audit entry records.
    pub action: AuditAction,
    /// UTC timestamp when the event occurred.
    pub timestamp: DateTime<Utc>,
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
    /// Additional unstructured details about the event.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<Metadata>,
}

impl Audit {
    /// Create a new audit record for the given action, timestamped to now.
    pub fn new(action: AuditAction) -> Self {
        Self {
            data: Data::new(),
            action,
            timestamp: Utc::now(),
            entity_id: None,
            redaction_id: None,
            policy_id: None,
            source_id: None,
            run_id: None,
            actor: None,
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
    pub fn with_details(mut self, details: Metadata) -> Self {
        self.details = Some(details);
        self
    }
}
