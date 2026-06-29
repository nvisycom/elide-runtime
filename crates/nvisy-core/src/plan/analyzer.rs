//! Top-level analyzer params.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::deduplication::DeduplicationParams;
use super::enricher::EnricherParams;
use super::label::LabelCatalogParams;
use super::recognizer::RecognizerParams;
use elide::recognition::Scope;

/// Full description of how to build an analyzer for one
/// request.
///
/// Engine compiles this into [`elide::detection::Analyzer<M>`] per
/// modality the request targets. The compile step
/// monomorphises each per-modality analyzer from the same
/// params — text picks up the text-applicable recognizers,
/// image picks up the image ones, etc.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
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
    /// Caller-asserted scope: languages, jurisdictions
    /// (`countries`), document labels, correlation id. Threaded
    /// into every recognizer's context. The `catalog` field on the
    /// scope is overwritten at engine compile time with the
    /// resolved [`LabelCatalogParams`] result; callers should
    /// leave it empty here.
    #[serde(default)]
    pub scope: Scope,
    /// Per-request entity-label catalog: builtins selected by
    /// name + custom inline schemas. Drives both what the
    /// analyzer is asked to emit (via [`Scope::with_catalog`]
    /// at compile time) and tag-based selector matching in the
    /// anonymizer.
    ///
    /// [`Scope::with_catalog`]: elide::recognition::Scope::with_catalog
    #[serde(default)]
    pub label_catalog: LabelCatalogParams,
}
