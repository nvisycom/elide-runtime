//! Audit trail records for data protection events.
//!
//! An [`Audit`] entry records an immutable event in the data protection
//! pipeline, carrying structured metadata for compliance.

use jiff::Timestamp;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use strum::{Display, EnumString};
use uuid::Uuid;

use nvisy_core::path::ContentSource;

/// Kind of auditable action recorded in an [`Audit`] entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Display, EnumString, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
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
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Audit {
    /// Content source identity and lineage.
    #[serde(flatten)]
    pub source: ContentSource,
    /// The kind of event this audit entry records.
    pub action: AuditAction,
    /// UTC timestamp when the event occurred.
    #[schemars(with = "String")]
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
}
