//! [`DetectionResult`] and friends: the artifact a detection pass
//! produces plus the lightweight listing/filter shapes.

use jiff::Timestamp;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::status::DetectionStatus;
use crate::document::provenance::AnyAudit;
use crate::phases::ingestion::ImportFile;

/// Immutable artifact produced by one detection pass.
///
/// Contains the per-document audits with `Execution::Pending`
/// decisions (so the caller can see what would be redacted under
/// the policy chain) plus the original [`ImportFile`] references —
/// a later [`Engine::redact`] call needs them to re-open content
/// for byte rewriting.
///
/// [`Engine::redact`]: super::super::Engine::redact
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DetectionResult {
    /// Unique identifier for this detection pass.
    pub id: Uuid,
    /// Identity of the actor who initiated the pass.
    pub actor_id: Uuid,
    /// Policies that were applied, in precedence order.
    pub policies: Vec<Uuid>,
    /// Original import references — needed by [`Engine::redact`].
    ///
    /// [`Engine::redact`]: super::super::Engine::redact
    pub imports: Vec<ImportFile>,
    /// Per-document audit trails. One entry per modality the
    /// pipeline produced.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub audits: Vec<AnyAudit>,
    /// Total entities detected across all documents.
    pub entities_detected: u64,
}

/// Full point-in-time snapshot of a detection pass.
///
/// `DetectionResult` is only present once the pass has reached a
/// terminal state with at least one successfully processed
/// document. While running, the caller still gets timestamps and
/// status.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DetectionSnapshot {
    /// Unique identifier for this detection pass.
    pub id: Uuid,
    /// Identity of the actor who initiated the pass.
    pub actor_id: Uuid,
    /// Current lifecycle state.
    pub status: DetectionStatus,
    /// When the pass was created.
    #[schemars(with = "String")]
    pub created_at: Timestamp,
    /// When recognisers actually started executing.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<String>")]
    pub started_at: Option<Timestamp>,
    /// When the pass reached a terminal state.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<String>")]
    pub completed_at: Option<Timestamp>,
    /// Detection result. `Some` once the pass has produced at
    /// least one audit (terminal status with `Succeeded` /
    /// `PartialFailure`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<DetectionResult>,
    /// Error description for failed passes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Lightweight summary of a detection pass for listing endpoints.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DetectionEntry {
    /// Unique identifier for this detection pass.
    pub id: Uuid,
    /// Identity of the actor who initiated the pass.
    pub actor_id: Uuid,
    /// Current overall status.
    pub status: DetectionStatus,
    /// When the pass was created.
    #[schemars(with = "String")]
    pub created_at: Timestamp,
    /// When recognisers started, if applicable.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<String>")]
    pub started_at: Option<Timestamp>,
    /// When the pass finished, if applicable.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<String>")]
    pub completed_at: Option<Timestamp>,
    /// Number of imports processed.
    pub import_count: usize,
    /// Total entities detected across all documents.
    pub entities_detected: u64,
}

/// Filter criteria for listing detection passes.
#[derive(Debug, Clone, Default)]
pub struct DetectionFilter {
    /// If set, only return passes with this status.
    pub status: Option<DetectionStatus>,
}
