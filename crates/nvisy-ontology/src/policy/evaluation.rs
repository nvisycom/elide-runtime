//! Policy evaluation outcome.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::record::Redaction;

/// Full outcome of evaluating a policy against a set of entities.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PolicyEvaluation {
    /// Identifier of the policy that was evaluated.
    pub policy_id: Uuid,
    /// Redactions produced by `Redact` rules.
    pub redactions: Vec<Redaction>,
    /// Entity IDs routed to human review by `Review` rules.
    pub pending_review: Vec<Uuid>,
    /// Entity IDs suppressed from output by `Suppress` rules.
    pub suppressed: Vec<Uuid>,
    /// Entity IDs blocked from processing by `Block` rules.
    pub blocked: Vec<Uuid>,
    /// Entity IDs that triggered alert notifications via `Alert` rules.
    pub alerted: Vec<Uuid>,
}
