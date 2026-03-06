//! Policy evaluation outcome.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::record::{RedactionDecision, RedactionRecord};

/// Full outcome of evaluating a policy against a set of entities.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PolicyEvaluation {
    /// Identifier of the policy that was evaluated.
    pub policy_id: Uuid,
    /// Pipeline-facing redaction decisions produced by `Redact` rules.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub decisions: Vec<RedactionDecision>,
    /// Audit-facing redaction records produced by `Redact` rules.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub records: Vec<RedactionRecord>,
    /// Entity IDs routed to human review by `Review` rules.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pending_review: Vec<Uuid>,
    /// Entity IDs suppressed from output by `Suppress` rules.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub suppressed: Vec<Uuid>,
    /// Entity IDs blocked from processing by `Block` rules.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub blocked: Vec<Uuid>,
    /// Entity IDs that triggered alert notifications via `Alert` rules.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub alerted: Vec<Uuid>,
}
