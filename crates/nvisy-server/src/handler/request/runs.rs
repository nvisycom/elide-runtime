//! Run request types.

use nvisy_engine::{Graph, RuntimeConfig};
use nvisy_ontology::policy::Policies;
use schemars::JsonSchema;
use serde::Deserialize;
use uuid::Uuid;

/// Request body for `POST /api/v1/runs` and `POST /api/v1/runs/scan`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct NewRun {
    /// Identifiers of previously uploaded content.
    pub content_ids: Vec<Uuid>,
    /// Policies to apply during processing.
    pub policies: Policies,
    /// Execution graph defining the pipeline DAG.
    pub graph: Graph,
    /// Per-request configuration overrides (optional).
    #[serde(default)]
    #[schemars(skip)]
    pub config: Option<RuntimeConfig>,
}
