//! Top-level analyzer params.

use elide_core::primitive::{CountryCode, Languages};
use elide_core::recognition::ScopeMetadata;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::annotation::AnyAnnotations;
use super::deduplication::DeduplicationParams;
use super::enricher::EnricherParams;
use super::recognizer::RecognizerParams;

/// Full description of how to build an analyzer for one
/// request.
///
/// Engine compiles this into `elide::detection::Analyzer<M>` per
/// modality the request targets. The compile step
/// monomorphises each per-modality analyzer from the same
/// params. Text picks up the text-applicable recognizers,
/// image picks up the image ones, etc.
///
/// The caller-asserted scope lives under [`scope`], a narrower
/// wire projection of `elide::recognition::Scope`: `languages`,
/// `countries`, and elide's own [`ScopeMetadata`] block for
/// free-form classification strings. `Scope`'s other two fields
/// are server-owned — `correlation_id` is minted per request,
/// and `catalog` is derived from the request's policy set — so
/// they don't appear on the wire.
///
/// [`scope`]: AnalyzerParams::scope
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AnalyzerParams {
    /// Recognizer slots.
    ///
    /// Pattern (at-most-one), ner, llm (each a list, identified
    /// by name).
    #[serde(default)]
    pub recognizers: RecognizerParams,
    /// Enricher slots: language, ocr, stt (each at-most-one).
    ///
    /// Enrichers run sequentially before recognition; the engine
    /// picks a canonical order (language → ocr → stt).
    #[serde(default)]
    pub enrichers: EnricherParams,
    /// Deduplication pipeline applied after recognition.
    ///
    /// Calibrate → reconcile → filter.
    #[serde(default)]
    pub deduplication: DeduplicationParams,
    /// Caller-asserted scope. See [`ScopeParams`].
    #[serde(default)]
    pub scope: ScopeParams,
    /// Per-modality region annotations.
    ///
    /// Empty by default. Each modality slot carries inclusions
    /// (candidate regions LLM-adjudicating recognizers fold into
    /// detection) and exclusions (protected regions the analyzer
    /// drops any overlapping entity from). Engine attaches each
    /// slot to the modality's pipeline at compile time.
    #[serde(default)]
    pub annotations: AnyAnnotations,
}

/// Caller-asserted scope for one request.
///
/// A narrower wire projection of `elide::recognition::Scope`:
/// `languages` and `countries` (typed, elide-native), plus
/// elide's [`ScopeMetadata`] block for free-form classification
/// strings (`tags`, `purpose`, `audience`). The engine assembles
/// this plus a server-minted `correlation_id` and a policy-derived
/// label catalog into the orchestrator's `Scope` at compile time.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ScopeParams {
    /// Caller-asserted languages for the analysis.
    ///
    /// Empty means the caller asserted none, leaving detection
    /// (if a language enricher runs) to fill in.
    #[serde(default)]
    pub languages: Languages,
    /// Caller-asserted jurisdictions.
    ///
    /// When non-empty, recognizers that carry per-rule country
    /// scopes skip rules that match none of them. An empty list
    /// means "any": rules that declare countries still run as a
    /// permissive fallback so callers who don't assert a
    /// jurisdiction don't lose detections.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub countries: Vec<CountryCode>,
    /// Free-form request context: document tags, request purpose,
    /// output audience. See elide's [`ScopeMetadata`].
    #[serde(default, skip_serializing_if = "scope_metadata_is_empty")]
    pub metadata: ScopeMetadata,
}

/// Whether the [`ScopeMetadata`] carries no assertions.
///
/// Used by [`ScopeParams::metadata`]'s `skip_serializing_if` to
/// keep the serialized `scope` block minimal when the caller
/// asserts nothing beyond the typed fields. Local to this module
/// because elide's `ScopeMetadata` doesn't ship an `is_empty` of
/// its own; a one-line helper is cheaper than a wrapper type.
fn scope_metadata_is_empty(metadata: &ScopeMetadata) -> bool {
    metadata.tags.is_empty() && metadata.purpose.is_none() && metadata.audience.is_empty()
}
