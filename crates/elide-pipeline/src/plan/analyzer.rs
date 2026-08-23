//! Top-level analyzer params.

use elide::primitive::{CountryCode, Languages, RasterMode};
use elide::recognition::ScopeMetadata;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::annotation::AnyAnnotations;

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
/// are server-owned: `correlation_id` is minted per request,
/// and `catalog` is derived from the request's policy set: so
/// they don't appear on the wire.
///
/// Everything else: the built-in pattern recognizer, every
/// wired NER and LLM recognizer, every wired enricher, the
/// dedup pipeline: always runs on every request. Deployment
/// controls the lineup via `Engine::with_ner` / `Engine::with_llm`;
/// per-request opt-outs aren't shipped.
///
/// [`scope`]: AnalyzerParams::scope
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AnalyzerParams {
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
    /// How container formats that carry both a text layer and
    /// page images (PDF today; DOCX, EPUB, TIFF as elide widens
    /// coverage) treat OCR. See [`RasterMode`] for the three states.
    ///
    /// The default [`RasterMode::Auto`] matches the codec's built-in
    /// behaviour: extract the text layer where present, render
    /// missing pages for OCR. `Force { dpi }` rasterises every
    /// page (real CPU / memory cost; opt-in when scanned pages
    /// need OCR even alongside a text layer). `Never` skips
    /// rendering entirely.
    #[serde(default)]
    pub raster_mode: RasterMode,
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

/// Whether a [`ScopeMetadata`] carries no assertions: all three
/// fields (`tags`, `purpose`, `audience`) empty.
///
/// Used as the `skip_serializing_if` for every wire type that
/// carries a `ScopeMetadata` (both [`ScopeParams::metadata`] here
/// and the engine-side `AuditContext::metadata`) so a serialized
/// block stays minimal when the caller asserts nothing beyond
/// the typed fields. Exposed because elide's `ScopeMetadata`
/// doesn't ship an `is_empty` of its own.
pub fn scope_metadata_is_empty(metadata: &ScopeMetadata) -> bool {
    metadata.tags.is_empty() && metadata.purpose.is_none() && metadata.audience.is_empty()
}
