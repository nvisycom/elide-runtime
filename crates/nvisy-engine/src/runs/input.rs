//! Inputs for [`crate::runs::start`].

use std::collections::HashMap;

use bytes::Bytes;
use nvisy_core::plan::AnalyzerSpec;
use serde::{Deserialize, Serialize};

use super::state::ResourceRef;

/// Per-call input to [`crate::runs::start`].
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartBatch {
    /// Policies to apply, by `(id, version)`. Engine resolves
    /// each against [`crate::policies::get`]; missing refs fail
    /// the start call with [`ErrorKind::NotFound`].
    ///
    /// [`ErrorKind::NotFound`]: nvisy_core::ErrorKind::NotFound
    pub policy_refs: Vec<ResourceRef>,
    /// Contexts to apply, by `(id, version)`. Same resolution path
    /// as [`policy_refs`](Self::policy_refs).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub context_refs: Vec<ResourceRef>,
    /// Input documents.
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
    /// into an [`elide::Analyzer`] per modality at start time.
    pub analyzer: AnalyzerSpec,
    /// Cap on per-doc analyses + applies running concurrently.
    /// `None` falls back to a sensible default at start time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub concurrency: Option<usize>,
}

/// One input document handed to [`crate::runs::start`].
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentInput {
    /// Raw bytes. Engine hands these to the elide codec at
    /// analyze time.
    pub bytes: Bytes,
    /// File extension the codec registry resolves on (e.g.
    /// `"txt"`, `"pdf"`, `"png"`). Case-insensitive, no leading
    /// dot.
    pub extension: String,
    /// Doc-level labels that gate
    /// [`DocumentPredicate::HasLabel`] policies. Caller authors
    /// these at upload time.
    ///
    /// [`DocumentPredicate::HasLabel`]: nvisy_core::policy::DocumentPredicate::HasLabel
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub descriptor_labels: Vec<String>,
    /// Doc-level metadata that gates
    /// [`DocumentPredicate::HasMetadata`] policies, merged with
    /// the per-request [`StartBatch::metadata`].
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub descriptor_metadata: HashMap<String, String>,
}
