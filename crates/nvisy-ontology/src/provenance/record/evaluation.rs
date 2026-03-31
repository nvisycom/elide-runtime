//! Policy evaluation outcome.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::redaction::RedactionRecord;

/// Full outcome of evaluating a policy against a set of entities.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PolicyEvaluation {
    /// Identifier of the policy that was evaluated.
    pub policy_id: Uuid,
    /// Redaction records produced by `Redact` rules.
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

impl PolicyEvaluation {
    /// Create an empty evaluation for the given policy.
    pub fn new(policy_id: Uuid) -> Self {
        Self {
            policy_id,
            records: Vec::new(),
            pending_review: Vec::new(),
            suppressed: Vec::new(),
            blocked: Vec::new(),
            alerted: Vec::new(),
        }
    }
}
