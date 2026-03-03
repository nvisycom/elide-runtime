//! Redact response types.

use schemars::JsonSchema;
use serde::Serialize;
use uuid::Uuid;

/// Response body for `POST /api/v1/redaction`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RedactionResponse {
    /// Unique run identifier.
    pub run_id: Uuid,
    /// Identifier of the redacted output content.
    pub output_id: Uuid,
    /// Per-source redaction summaries as opaque JSON.
    pub summaries: serde_json::Value,
    /// Audit trail entries as opaque JSON.
    pub audits: serde_json::Value,
}
