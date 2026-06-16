//! Request bodies for `/detections` endpoints.

use nvisy_engine::core::ingestion::ImportFile;
use nvisy_engine::detection::{DetectionInput, DetectionPlan, DetectionStatus};
use nvisy_engine::policy::Policy;
use schemars::JsonSchema;
use serde::Deserialize;
use uuid::Uuid;

use crate::handler::request::pagination::Pagination;

/// Body for `POST /detections`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct NewDetection {
    /// Policies to apply, in precedence order
    /// (index 0 is highest precedence).
    pub policies: Vec<Policy>,
    /// Content sources to ingest at the start of the pass.
    pub imports: Vec<ImportFile>,
    /// Per-phase behaviour knobs.
    #[serde(default)]
    pub plan: DetectionPlan,
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
