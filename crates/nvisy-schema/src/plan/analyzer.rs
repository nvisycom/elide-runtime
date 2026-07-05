//! Top-level analyzer params.

use elide_core::primitive::{CountryCode, Languages};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::deduplication::DeduplicationParams;
use super::enricher::EnricherParams;
use super::label::LabelCatalogParams;
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
/// The caller-asserted scope lives under [`scope`], a single
/// nested object grouping the four knobs the engine assembles
/// into an `elide::recognition::Scope` at compile time
/// ([`languages`], [`countries`], [`labels`], [`label_catalog`]).
/// `Scope`'s fifth knob, `correlation_id`, is server-minted per
/// request and never appears on the wire.
///
/// [`scope`]: AnalyzerParams::scope
/// [`languages`]: ScopeParams::languages
/// [`countries`]: ScopeParams::countries
/// [`labels`]: ScopeParams::labels
/// [`label_catalog`]: ScopeParams::label_catalog
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
    /// Caller-asserted scope.
    ///
    /// Languages, jurisdictions, document labels, and the
    /// per-request entity-label catalog. Engine assembles this
    /// (plus a server-minted correlation id) into an
    /// `elide::recognition::Scope` at compile time.
    #[serde(default)]
    pub scope: ScopeParams,
}

/// Caller-asserted scope for one request.
///
/// Mirrors the wire-visible knobs of `elide::recognition::Scope`.
/// The engine assembles this plus a server-minted
/// `correlation_id` into the orchestrator's `Scope` at compile
/// time.
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
    /// Document-level classification labels.
    ///
    /// E.g. `"medical"`, `"gdpr-request"`. Recognizers may use
    /// these to bias their behaviour for domain-specific terms;
    /// those that don't ignore the field.
    ///
    /// Distinct from [`label_catalog`]: these classify the
    /// *document*, whereas the catalog names the entity *types*
    /// to emit.
    ///
    /// [`label_catalog`]: ScopeParams::label_catalog
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub labels: Vec<String>,
    /// Per-request entity-label catalog.
    ///
    /// Builtins selected by name + custom inline schemas. Drives
    /// what recognizers are asked to emit and tag-based selector
    /// matching in the anonymizer. Engine resolves this into the
    /// assembled `elide::recognition::Scope`'s `catalog` field at
    /// compile time.
    #[serde(default)]
    pub label_catalog: LabelCatalogParams,
}
