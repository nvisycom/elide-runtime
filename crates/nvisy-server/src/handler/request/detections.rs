//! Request bodies for `/detections` endpoints.

use std::collections::HashMap;

use nvisy_core::plan::AnalyzerParams;
use nvisy_engine::runs::{DocumentInput, ResourceRef, StartBatch};
use schemars::JsonSchema;
use serde::Deserialize;
use uuid::Uuid;

use super::analyzer::AnalyzerOverrides;
use super::pagination::Pagination;

/// Body for `POST /detections`. Documents are file ids
/// previously uploaded via `POST /files`; the server resolves
/// them at start time.
///
/// The `analyzer` field carries per-request *overrides* on top
/// of the deployment's default [`AnalyzerParams`] — clients that
/// omit it inherit the default. See [`AnalyzerOverrides`] for the
/// per-field merge semantics.
#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(default, rename_all = "camelCase")]
pub struct NewDetection {
    /// Policies to apply, in precedence order.
    pub policy_refs: Vec<ResourceRef>,
    /// Contexts to apply.
    pub context_refs: Vec<ResourceRef>,
    /// File ids to analyze. Must exist for the calling actor.
    pub documents: Vec<Uuid>,
    /// Per-request metadata merged with each document's
    /// descriptor at policy-evaluation time.
    pub metadata: HashMap<String, String>,
    /// Analyzer overrides. Field-by-field merge into the
    /// deployment default; omitted fields inherit the default.
    pub analyzer: AnalyzerOverrides,
    /// Per-doc concurrency cap. `None` for the engine default.
    pub concurrency: Option<usize>,
}

impl NewDetection {
    /// Project the request into the typed engine input,
    /// resolving the analyzer params against the server's
    /// configured default.
    pub fn into_engine_input(self, analyzer_default: &AnalyzerParams) -> StartBatch {
        StartBatch {
            policy_refs: self.policy_refs,
            context_refs: self.context_refs,
            documents: self
                .documents
                .into_iter()
                .map(|file_id| DocumentInput { file_id })
                .collect(),
            metadata: self.metadata,
            analyzer: self.analyzer.resolve(analyzer_default),
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
