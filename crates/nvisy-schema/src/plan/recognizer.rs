//! Recognizer params: per-kind slots inside an [`AnalyzerParams`].
//!
//! Three recognizer kinds (pattern, NER, LLM):
//!
//! - **Pattern** is at-most-one, per-request. Single
//!   regex/dictionary engine per analyzer; multi-pattern means
//!   accumulating into one instance's pattern list, not running
//!   two engines. Callers may inline custom regex rules
//!   ([`CustomPatternRule`]) and dictionaries
//!   ([`CustomDictionary`]) alongside the shipped `builtins`;
//!   the engine compiles them per request.
//! - **NER** is a **deployment-owned lineup** selected by a
//!   [`ProviderSelection`]. Provider, model, and (future)
//!   credentials live in the deployment config; the wire opts in
//!   (`true`), opts out (`false`), names a subset by recognizer
//!   name, or leaves it to the default (`None`, softly-on:
//!   attach when the deployment has any NER configured, skip
//!   otherwise).
//! - **LLM** is the same shape as NER.
//!
//! Rationale for the NER/LLM shape: policies stay portable
//! across deployments, the operator controls model choice and
//! rate-limits, and tenants cannot leak credentials onto the
//! wire.
//!
//! [`AnalyzerParams`]: super::AnalyzerParams

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::pattern::{CustomDictionary, CustomPatternRule};

/// Recognizer slots an analyzer can fill.
///
/// Pattern is at-most-one (per-request); NER and LLM are
/// deployment-owned lineups each selected by a
/// [`ProviderSelection`].
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RecognizerParams {
    /// Built-in pattern + dictionary recognizer (`elide-pattern`).
    ///
    /// At most one per analyzer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pattern: Option<PatternRecognizerParams>,
    /// Select which of the deployment's NER recognizers to run.
    ///
    /// See [`ProviderSelection`] for the shape. `None` is the
    /// softly-on default: attach every configured recognizer if
    /// the deployment has any, skip silently otherwise.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ner: Option<ProviderSelection>,
    /// Select which of the deployment's LLM recognizers to run.
    ///
    /// Same shape as [`ner`]. The lineup is additionally filtered
    /// by declared modality — only recognizers whose `modalities`
    /// list contains the analyzer's modality attach.
    ///
    /// [`ner`]: RecognizerParams::ner
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub llm: Option<ProviderSelection>,
}

/// How to pick recognizers out of a deployment-configured lineup.
///
/// Untagged on the wire: `true` / `false` / a list of names.
///
/// - `All(true)`: explicit opt-in. Attaches every configured
///   recognizer; fails the analyzer compile if the lineup is
///   empty.
/// - `All(false)`: explicit opt-out. Skips the lineup entirely.
/// - `Only(names)`: attach only the named recognizers. An empty
///   list and any unknown name fail the analyzer compile.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum ProviderSelection {
    /// Whole-lineup toggle.
    All(bool),
    /// Allowlist by recognizer name.
    Only(Vec<String>),
}

/// Params for the `elide-pattern` recognizer.
#[derive(Debug, Clone, Default, PartialEq)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PatternRecognizerParams {
    /// Load every pattern + dictionary shipped with `elide-pattern`.
    ///
    /// Implies the country-scoped jurisdictional pattern packs
    /// are active for the scope's jurisdictions.
    #[serde(default)]
    pub builtins: bool,
    /// Enable per-label context-keyword boosting.
    ///
    /// Wraps the bare pattern recognizer in elide's
    /// `Enhanced<PatternRecognizer>` layer so per-label context
    /// keywords boost low-confidence matches before they leave
    /// the recognizer.
    #[serde(default)]
    pub context_enhanced: bool,
    /// Caller-inlined regex rules.
    ///
    /// Compiled per-request. See [`CustomPatternRule`] for the
    /// shape; the engine bounds request-level cost with a rule-
    /// count cap and a per-regex NFA-size limit at compile time,
    /// on top of the deserialize-time source-length cap in
    /// [`MAX_REGEX_SOURCE_LEN`].
    ///
    /// [`MAX_REGEX_SOURCE_LEN`]: super::MAX_REGEX_SOURCE_LEN
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub custom: Vec<CustomPatternRule>,
    /// Caller-inlined literal-term dictionaries.
    ///
    /// Compiled per-request into a shared Aho-Corasick automaton.
    /// Same rule-count cap as `custom` applies at compile time.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub custom_dictionaries: Vec<CustomDictionary>,
}
