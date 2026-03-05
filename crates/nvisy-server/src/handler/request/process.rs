//! Process request types.

use nvisy_engine::Graph;
use nvisy_engine::pipeline::Policies;
use nvisy_registry::{ActorId, ContentId};
use schemars::JsonSchema;
use serde::Deserialize;

/// Request body for `POST /api/v1/process/*` endpoints.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ProcessRequest {
    /// Actor identity for registry lookups.
    pub actor_id: ActorId,
    /// Identifiers of previously uploaded content.
    pub content_ids: Vec<ContentId>,
    /// Policies to apply during processing.
    pub policies: Policies,
    /// Execution graph defining the pipeline DAG.
    pub graph: Graph,
}
