//! Human-in-the-loop review types.

use jiff::Timestamp;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use strum::{Display, EnumString};
use uuid::Uuid;

/// Status of a human review on a redaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[derive(Display, EnumString, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
#[non_exhaustive]
pub enum ReviewStatus {
    /// Awaiting human review.
    Pending,
    /// A human reviewer approved the redaction.
    Approved,
    /// A human reviewer rejected the redaction.
    Rejected,
    /// Automatically approved by policy (no human review required).
    AutoApproved,
}

/// A review decision recorded against a redaction, including versioning.
///
/// Present on an [`AuditEntry`](super::AuditEntry) only when the
/// redaction has been reviewed (or is pending review). Absent for
/// entries that have not entered the review workflow.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReviewDecision {
    /// Version of the audit entry at review time (starts at 1).
    pub version: u32,
    /// Outcome of the review.
    pub status: ReviewStatus,
    /// Identifier of the reviewer (human or service account).
    pub reviewer_id: Uuid,
    /// When the review decision was made.
    #[schemars(with = "String")]
    pub timestamp: Timestamp,
    /// Optional reason for the decision.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}
