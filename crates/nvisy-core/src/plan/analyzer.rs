//! Top-level analyzer params.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::deduplication::DeduplicationParams;
use super::enricher::EnricherParams;
use super::recognizer::RecognizerParams;
use super::scope::ScopeParams;
use crate::schema::LabelSchema;

/// Full description of how to build an analyzer for one
/// request.
///
/// Engine compiles this into [`elide::detection::Analyzer<M>`] per
/// modality the request targets. The compile step
/// monomorphises each per-modality analyzer from the same
/// params — text picks up the text-applicable recognizers,
/// image picks up the image ones, etc.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AnalyzerParams {
    /// Recognizer slots: pattern (at-most-one), ner, llm
    /// (each a list, identified by name).
    #[serde(default)]
    pub recognizers: RecognizerParams,
    /// Enricher slots: language, ocr, stt (each at-most-one).
    /// Enrichers run sequentially before recognition; the
    /// engine picks a canonical order (language → ocr → stt).
    #[serde(default)]
    pub enrichers: EnricherParams,
    /// Deduplication pipeline applied after recognition:
    /// calibrate, fuse, resolve, filter. Defaults assemble the
    /// canonical pipeline at compile time.
    #[serde(default)]
    pub deduplication: DeduplicationParams,
    /// Caller-asserted scope (languages, jurisdictions).
    /// Threaded into every recognizer's context.
    #[serde(default)]
    pub scope: ScopeParams,
    /// Per-request entity-label catalog. Drives tag-based
    /// selector matching downstream in the anonymizer compile.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub label_catalog: Vec<LabelSchema>,
}
