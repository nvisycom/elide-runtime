//! Execute response types.

use nvisy_engine::engine::{
    Audit, DetectionOutput, EngineOutput, PolicyEvaluation, RedactionSummary, RunOutput,
};
use schemars::JsonSchema;
use serde::Serialize;
use uuid::Uuid;

/// Response body for `POST /api/v1/execute`.
#[derive(Debug, Serialize, JsonSchema)]
pub struct ExecuteResponse {
    /// Unique run identifier.
    pub run_id: Uuid,
    /// Detection output (entities, source, timing).
    pub detection: DetectionOutput,
    /// Policy evaluation breakdown (redactions, reviews, suppressions).
    pub evaluation: PolicyEvaluation,
    /// Per-source redaction summaries.
    pub summaries: Vec<RedactionSummary>,
    /// Immutable audit trail.
    pub audits: Vec<Audit>,
    /// Per-node DAG execution results.
    pub run_output: RunOutput,
}

impl From<EngineOutput> for ExecuteResponse {
    fn from(out: EngineOutput) -> Self {
        Self {
            run_id: out.run_id,
            detection: out.detection,
            evaluation: out.evaluation,
            summaries: out.summaries,
            audits: out.audits,
            run_output: out.run_output,
        }
    }
}
