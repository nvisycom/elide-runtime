//! [`RedactionResult`] and friends.

use jiff::Timestamp;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::status::RedactionStatus;
use crate::provenance::AnyAudit;

/// Artifact produced by one redaction pass.
///
/// Contains the per-document audits with `Execution` populated
/// (applied / suppressed / failed) plus aggregate counters.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RedactionResult {
    /// Unique identifier for this redaction pass.
    pub id: Uuid,
    /// Detection pass this redaction was applied against.
    pub detection_id: Uuid,
    /// Identity of the actor who initiated the pass.
    pub actor_id: Uuid,
    /// Per-document audits with `Execution` populated.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub audits: Vec<AnyAudit>,
    /// Total redactions actually applied across all documents.
    pub redactions_applied: u64,
}

/// Full snapshot of a redaction pass.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RedactionSnapshot {
    pub id: Uuid,
    pub detection_id: Uuid,
    pub actor_id: Uuid,
    pub status: RedactionStatus,
    #[schemars(with = "String")]
    pub created_at: Timestamp,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<String>")]
    pub started_at: Option<Timestamp>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<String>")]
    pub completed_at: Option<Timestamp>,
    /// Result is `Some` once the pass has reached a terminal
    /// state with at least one audit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<RedactionResult>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Lightweight summary of a redaction pass for listing endpoints.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RedactionEntry {
    pub id: Uuid,
    pub detection_id: Uuid,
    pub actor_id: Uuid,
    pub status: RedactionStatus,
    #[schemars(with = "String")]
    pub created_at: Timestamp,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<String>")]
    pub started_at: Option<Timestamp>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<String>")]
    pub completed_at: Option<Timestamp>,
    pub redactions_applied: u64,
}

/// Filter criteria for listing redaction passes.
#[derive(Debug, Clone, Default)]
pub struct RedactionFilter {
    /// If set, only return passes with this status.
    pub status: Option<RedactionStatus>,
    /// If set, only return passes for this detection.
    pub detection_id: Option<Uuid>,
}
