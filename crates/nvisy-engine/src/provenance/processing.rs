//! Data for deterministic processing operations.

use derive_builder::Builder;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Data specific to deterministic processing operations
/// (pattern matching, redaction, etc.).
///
/// Duration and error are tracked on [`FileAuditEntry`](super::FileAuditEntry).
#[derive(Debug, Clone, Default)]
#[derive(Builder, Serialize, Deserialize, JsonSchema)]
#[builder(
    name = "ProcessingActionBuilder",
    pattern = "owned",
    setter(into = false, strip_option, prefix = "with")
)]
#[serde(rename_all = "camelCase")]
pub struct ProcessingAction {
    /// Number of items processed.
    #[builder(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub items_count: Option<u64>,
    /// Number of items that matched.
    #[builder(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub matched_count: Option<u64>,
    /// Entity identifiers correlated with this processing action.
    #[builder(default)]
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub entity_ids: Vec<Uuid>,
    /// Redaction identifiers correlated with this processing action.
    #[builder(default)]
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub redaction_ids: Vec<Uuid>,
}
