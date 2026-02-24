//! Human-in-the-loop review types.

use jiff::Timestamp;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Status of a human review on a redaction decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[derive(JsonSchema)]
#[serde(rename_all = "snake_case")]
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

/// A review decision recorded against a redaction.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[derive(JsonSchema)]
pub struct ReviewDecision {
    /// Outcome of the review.
    pub status: ReviewStatus,
    /// Identifier of the reviewer (human or service account).
    pub reviewer_id: String,
    /// When the review decision was made.
    #[schemars(with = "String")]
    pub timestamp: Timestamp,
    /// Optional reason for the decision.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}
