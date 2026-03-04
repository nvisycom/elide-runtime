//! Execute response types.

use nvisy_engine::pipeline::{
    DetectionOutput, EngineOutput, PolicyEvaluation, RedactionMap, RedactionSummary,
    RunOutput,
};
use nvisy_engine::provenance::FileAudit;
use schemars::JsonSchema;
use serde::Serialize;
use uuid::Uuid;

/// Response body for `POST /api/v1/execute`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ExecuteResponse {
    /// Unique run identifier.
    pub run_id: Uuid,
    /// Detection output (entities, source, timing).
    pub detection: DetectionOutput,
    /// Policy evaluation breakdown (redactions, reviews, suppressions).
    pub evaluation: PolicyEvaluation,
    /// Per-source redaction summaries.
    pub summaries: Vec<RedactionSummary>,
    /// Per-file processing logs.
    pub file_audits: Vec<FileAudit>,
    /// Redaction mapping artifacts.
    pub redaction_maps: Vec<RedactionMap>,
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
            file_audits: out.file_audits,
            redaction_maps: out.redaction_maps,
            run_output: out.run_output,
        }
    }
}
