//! Top-level analyzer spec.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::deduplication::DeduplicationSpec;
use super::enricher::EnricherSpec;
use super::recognizer::RecognizerSpec;
use super::scope::ScopeSpec;
use crate::schema::LabelSchema;

/// Full description of how to build an analyzer for one request.
///
/// Engine compiles this into [`elide::Analyzer<M>`] per modality
/// the request targets. The compile step monomorphises each
/// per-modality analyzer from the same spec — text picks up the
/// text-applicable recognizers, image picks up the image ones, etc.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AnalyzerSpec {
    /// Recognizers to run during the recognition phase. Run
    /// concurrently within a single chunk.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub recognizers: Vec<RecognizerSpec>,
    /// Enrichers to run before recognition, in order. Each sees the
    /// working context the previous one wrote.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub enrichers: Vec<EnricherSpec>,
    /// Deduplication pipeline applied after recognition: calibrate,
    /// fuse, resolve, filter. Defaults assemble the canonical
    /// pipeline at compile time.
    #[serde(default)]
    pub deduplication: DeduplicationSpec,
    /// Caller-asserted scope (languages, jurisdictions). Threaded
    /// into every recognizer's context.
    #[serde(default)]
    pub scope: ScopeSpec,
    /// Per-request entity-label catalog. Drives tag-based selector
    /// matching downstream in the anonymizer compile.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub label_catalog: Vec<LabelSchema>,
}
