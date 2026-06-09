//! Request bodies for `/detections` endpoints.

use nvisy_engine::core::Plan;
use nvisy_engine::phases::ingestion::ImportFile;
use nvisy_engine::pipeline::{DetectionInput, DetectionStatus};
use schemars::JsonSchema;
use serde::Deserialize;
use uuid::Uuid;

use crate::handler::request::pagination::Pagination;

/// Body for `POST /detections`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct NewDetection {
    /// Previously uploaded policies to apply, in precedence
    /// order (index 0 is highest precedence).
    #[serde(default)]
    pub policies: Vec<Uuid>,
    /// Content sources to ingest at the start of the pass.
    pub imports: Vec<ImportFile>,
    /// Per-phase behaviour knobs.
    #[serde(default)]
    pub plan: Plan,
}

impl NewDetection {
    /// Project the request into the typed engine input.
    pub fn into_engine_input(self, actor_id: Uuid) -> DetectionInput {
        DetectionInput {
            actor_id,
            policies: self.policies,
            imports: self.imports,
            plan: self.plan,
        }
    }
}

/// Query parameters for `GET /detections`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DetectionQuery {
    /// Optional status filter.
    #[serde(default)]
    pub status: Option<DetectionStatus>,
    /// Pagination knobs.
    #[serde(flatten)]
    pub pagination: Pagination,
}
