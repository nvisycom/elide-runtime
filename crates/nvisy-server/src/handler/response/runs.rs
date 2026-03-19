//! Run response types.

use nvisy_engine::provenance::{Audit, PolicyEvaluation, RedactionMap};
use nvisy_engine::pipeline::{EngineOutput, RunSnapshot, RunSummary};
use nvisy_ontology::entity::DetectionOutput;
use nvisy_ontology::policy::RedactionSummary;
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
pub struct Run {
    /// Full run snapshot.
    #[serde(flatten)]
    pub run: RunSnapshot,
}

/// Response body for `GET /api/v1/runs`.
#[derive(Debug, Serialize, JsonSchema)]
pub struct RunList {
    /// List of run summaries.
    pub runs: Vec<RunSummary>,
}
