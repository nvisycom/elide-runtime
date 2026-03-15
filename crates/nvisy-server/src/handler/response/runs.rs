//! Run response types.

use nvisy_engine::{RunSnapshot, RunSummary};
use schemars::JsonSchema;
use serde::Serialize;
use uuid::Uuid;

/// Response body for `POST /api/v1/runs` and `POST /api/v1/runs/scan`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RunResult {
    /// Unique run identifier.
    pub run_id: Uuid,
    /// Per-source result summaries as opaque JSON.
    pub summaries: serde_json::Value,
    /// Audit trail entries as opaque JSON.
    pub audits: serde_json::Value,
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
