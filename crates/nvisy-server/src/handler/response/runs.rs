//! Run response types.

use nvisy_engine::pipeline::{
    Audit, DetectionOutput, EngineOutput, PolicyEvaluation, RedactionMap, RedactionSummary,
    RunSnapshot, RunSummary,
};

use crate::handler::request::Page;

/// Response body for `GET /api/v1/runs`.
pub type RunList = Page<RunSummary>;
use schemars::JsonSchema;
use serde::Serialize;
use uuid::Uuid;

/// Response body for `POST /api/v1/runs`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RunResult {
    /// Unique run identifier.
    pub run_id: Uuid,
    /// Full detection result (entities, sensitivity, risk).
    pub detection: DetectionOutput,
    /// Policy evaluation breakdown (decisions, reviews, suppressions).
    pub evaluation: PolicyEvaluation,
    /// Per-source redaction summaries.
    pub summaries: Vec<RedactionSummary>,
    /// Per-file processing audit trails.
    pub audits: Vec<Audit>,
    /// Redaction mapping artifacts.
    pub redaction_maps: Vec<RedactionMap>,
}

impl From<EngineOutput> for RunResult {
    fn from(output: EngineOutput) -> Self {
        Self {
            run_id: output.run_id,
            detection: output.detection,
            evaluation: output.evaluation,
            summaries: output.summaries,
            audits: output.file_audits,
            redaction_maps: output.redaction_maps,
        }
    }
}

/// Response body for `GET /api/v1/runs/{id}`.
#[derive(Debug, Serialize, JsonSchema)]
pub struct RunDetail {
    /// Full run snapshot.
    #[serde(flatten)]
    pub run: RunSnapshot,
}
