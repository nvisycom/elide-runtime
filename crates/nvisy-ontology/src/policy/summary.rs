//! Per-source redaction summary.

use nvisy_core::content::ContentSource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Summary of redactions applied to a single content source.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RedactionSummary {
    /// The content source these counts apply to.
    pub source: ContentSource,
    /// Identifier of the policy that produced this summary.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub policy_id: Option<Uuid>,
    /// Number of redactions successfully applied.
    pub redactions_applied: usize,
    /// Number of redactions skipped (e.g. due to review holds or errors).
    pub redactions_skipped: usize,
}
