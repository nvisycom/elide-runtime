//! Execute response types.

use nvisy_engine::engine::{EngineOutput, RedactionSummary, RunOutput};
use schemars::JsonSchema;
use serde::Serialize;
use uuid::Uuid;

/// Response body for `POST /api/v1/execute`.
#[derive(Debug, Serialize, JsonSchema)]
pub struct ExecuteResponse {
    /// Unique run identifier.
    pub run_id: Uuid,
    /// Detection output as opaque JSON (DetectionOutput lacks JsonSchema).
    pub detection: serde_json::Value,
    /// Policy evaluation as opaque JSON (PolicyEvaluation lacks JsonSchema).
    pub evaluation: serde_json::Value,
    /// Per-source redaction summaries.
    pub summaries: Vec<RedactionSummary>,
    /// Audit trail entries as opaque JSON (Audit uses flatten).
    pub audits: serde_json::Value,
    /// Per-node DAG execution results.
    pub run_output: RunOutput,
}

impl From<EngineOutput> for ExecuteResponse {
    fn from(out: EngineOutput) -> Self {
        Self {
            run_id: out.run_id,
            detection: serde_json::to_value(&out.detection).unwrap_or_default(),
            evaluation: serde_json::to_value(&out.evaluation).unwrap_or_default(),
            summaries: out.summaries,
            audits: serde_json::to_value(&out.audits).unwrap_or_default(),
            run_output: out.run_output,
        }
    }
}
