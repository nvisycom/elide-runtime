//! Inputs for [`crate::runs::start`].

use std::collections::HashMap;

use nvisy_core::plan::AnalyzerParams;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::state::ResourceRef;

/// Per-call input to [`crate::runs::start`].
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct StartBatch {
    /// Policies to apply, by `(id, version)`. Engine resolves
    /// each against [`crate::PolicyRegistry::get_policy`]; missing
    /// refs fail the start call with [`ErrorKind::NotFound`].
    ///
    /// [`ErrorKind::NotFound`]: nvisy_core::ErrorKind::NotFound
    pub policy_refs: Vec<ResourceRef>,
    /// Contexts to apply, by `(id, version)`. Same resolution path
    /// as [`policy_refs`](Self::policy_refs).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub context_refs: Vec<ResourceRef>,
    /// Input documents — each is a file id previously uploaded
    /// via [`crate::FileRegistry::put_file`]. Engine resolves
    /// every id at start time; missing files fail the call with
    /// [`ErrorKind::NotFound`].
    ///
    /// [`ErrorKind::NotFound`]: nvisy_core::ErrorKind::NotFound
    pub documents: Vec<DocumentInput>,
    /// Per-request metadata merged with each document's
    /// descriptor at [`DocumentPredicate::HasMetadata`] evaluation
    /// time. Per-request keys override descriptor keys on
    /// conflict.
    ///
    /// [`DocumentPredicate::HasMetadata`]: nvisy_core::policy::DocumentPredicate::HasMetadata
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub metadata: HashMap<String, String>,
    /// Recognition plan — which recognizers + enrichers run, the
    /// dedup pipeline, the request scope. Engine compiles this
    /// into an [`elide::detection::Analyzer`] per modality at start time.
    pub analyzer: AnalyzerParams,
    /// Cap on per-doc analyses + applies running concurrently.
    /// `None` falls back to a sensible default at start time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub concurrency: Option<usize>,
}

/// One input document handed to [`crate::runs::start`]. Files
/// are uploaded once via [`crate::FileRegistry`]; runs reference
/// them by id and inherit their extension + descriptor labels +
/// metadata.
///
/// [`crate::FileRegistry`]: crate::FileRegistry
#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DocumentInput {
    /// File the run analyses + redacts. Must exist under
    /// `(actor_id, file_id)` in the engine's files keyspace.
    pub file_id: Uuid,
}
