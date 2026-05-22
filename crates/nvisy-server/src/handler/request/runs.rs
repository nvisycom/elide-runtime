//! Run request types.

use nvisy_engine::pipeline::{RunStatus, RuntimeConfig};
use nvisy_engine::workflow::Graph;
use nvisy_ontology::policy::PolicyRef;
use schemars::JsonSchema;
use serde::Deserialize;

use super::Pagination;

/// Request body for `POST /runs`.
///
/// Content identifiers are specified on [`Import`] nodes within the
/// graph, not as a top-level field. Each [`PolicyRef`] references a
/// previously uploaded policy and carries the precedence at which it
/// should layer relative to other refs in the same run.
///
/// [`Import`]: nvisy_engine::workflow::ImportFile
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct NewRun {
    /// Previously uploaded policies to apply, tagged with precedence.
    pub policies: Vec<PolicyRef>,
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
