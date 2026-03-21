//! Run request types.

use nvisy_engine::graph::Graph;
use nvisy_engine::pipeline::RuntimeConfig;
use nvisy_ontology::policy::Policies;
use schemars::JsonSchema;
use serde::Deserialize;

/// Request body for `POST /api/v1/runs`.
///
/// Content identifiers are specified on [`Import`] nodes within the
/// graph, not as a top-level field.
///
/// [`Import`]: nvisy_engine::graph::Import
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
}
