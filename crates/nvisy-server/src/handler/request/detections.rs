//! Request bodies for `/detections` endpoints.

use std::collections::HashMap;

use nvisy_core::plan::AnalyzerSpec;
use nvisy_engine::runs::{DocumentInput, StartBatch};
use schemars::JsonSchema;
use serde::Deserialize;
use uuid::Uuid;

use super::pagination::Pagination;
use super::refs::ResourceRef;

/// Body for `POST /detections`. Documents are file ids
/// previously uploaded via `POST /files`; the server resolves
/// them at start time.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct NewDetection {
    /// Policies to apply, in precedence order.
    pub policy_refs: Vec<ResourceRef>,
    /// Contexts to apply.
    #[serde(default)]
    pub context_refs: Vec<ResourceRef>,
    /// File ids to analyze. Must exist for the calling actor.
    pub documents: Vec<Uuid>,
    /// Per-request metadata merged with each document's
    /// descriptor at policy-evaluation time.
    #[serde(default)]
    pub metadata: HashMap<String, String>,
    /// Recognition plan — recognizers, enrichers, dedup
    /// pipeline, scope.
    pub analyzer: AnalyzerSpec,
    /// Per-doc concurrency cap. `None` for the engine default.
    #[serde(default)]
    pub concurrency: Option<usize>,
}

impl NewDetection {
    /// Project the request into the typed engine input.
    pub fn into_engine_input(self) -> StartBatch {
        StartBatch {
            policy_refs: self.policy_refs.into_iter().map(Into::into).collect(),
            context_refs: self.context_refs.into_iter().map(Into::into).collect(),
            documents: self
                .documents
                .into_iter()
                .map(|file_id| DocumentInput { file_id })
                .collect(),
            metadata: self.metadata,
            analyzer: self.analyzer,
            concurrency: self.concurrency,
        }
    }
}

/// Query parameters for `GET /detections`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DetectionQuery {
    /// Pagination knobs.
    #[serde(flatten)]
    pub pagination: Pagination,
}
