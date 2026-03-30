//! Run request types.

use nvisy_engine::pipeline::{RunStatus, RuntimeConfig};
use nvisy_ontology::policy::Policies;
use nvisy_ontology::workflow::Graph;
use schemars::JsonSchema;
use serde::Deserialize;

use super::Pagination;

/// Request body for `POST /api/v1/runs`.
///
/// Content identifiers are specified on [`Import`] nodes within the
/// graph, not as a top-level field.
///
/// [`Import`]: nvisy_ontology::workflow::ImportFile
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct NewRun {
    /// Policies to apply during processing.
    pub policies: Policies,
    /// Execution graph defining the pipeline DAG.
    pub graph: Graph,
    /// Per-request configuration overrides (optional).
    #[serde(default)]
    #[schemars(skip)]
    pub config: Option<RuntimeConfig>,
    /// When `true`, evaluate detection and policy rules but skip
    /// validation and export. Returns the redaction plan without
    /// modifying or exporting content.
    #[serde(default)]
    pub dry_run: bool,
}

/// Query parameters for listing runs.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RunQuery {
    /// Filter by run status (e.g. `running`, `succeeded`).
    #[serde(default)]
    pub status: Option<RunStatus>,
    /// Pagination parameters.
    #[serde(flatten)]
    pub pagination: Pagination,
}
