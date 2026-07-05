//! Recognizer params: per-kind slots inside an [`AnalyzerParams`].
//!
//! Three recognizer kinds (pattern, NER, LLM):
//!
//! - **Pattern** is at-most-one, per-request. Single
//!   regex/dictionary engine per analyzer; multi-pattern means
//!   accumulating into one instance's pattern list, not running
//!   two engines.
//! - **NER** is a **deployment-owned lineup** gated by a
//!   boolean toggle. Provider, model, and (future) credentials
//!   live in the deployment config; the wire only opts in or
//!   out.
//! - **LLM** is the same shape as NER: a deployment-owned
//!   lineup gated by a boolean toggle.
//!
//! Rationale for the NER/LLM shape: policies stay portable
//! across deployments, the operator controls model choice and
//! rate-limits, and tenants cannot leak credentials onto the
//! wire.
//!
//! [`AnalyzerParams`]: super::AnalyzerParams

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Recognizer slots an analyzer can fill.
///
/// Pattern is at-most-one (per-request); NER and LLM are
/// deployment-owned lineups each gated by a boolean toggle.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RecognizerParams {
    /// Built-in pattern + dictionary recognizer (`elide-pattern`).
    ///
    /// At most one per analyzer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pattern: Option<PatternRecognizerParams>,
    /// Run the deployment's NER recognizer lineup.
    ///
    /// `false` skips NER recognition entirely; `true` attaches
    /// every deployment-configured recognizer. When the
    /// deployment has no NER recognizers configured, `true` fails
    /// the analyzer compile with a `Validation` error.
    #[serde(default, skip_serializing_if = "is_false")]
    pub ner: bool,
    /// Run the deployment's LLM recognizer lineup.
    ///
    /// `false` skips LLM recognition entirely; `true` attaches
    /// every deployment-configured recognizer whose declared
    /// modalities match the analyzer's modality. When the
    /// deployment has no LLM recognizers configured for this
    /// modality, `true` fails the analyzer compile with a
    /// `Validation` error.
    #[serde(default, skip_serializing_if = "is_false")]
    pub llm: bool,
}

fn is_false(b: &bool) -> bool {
    !*b
}

/// Params for the `elide-pattern` recognizer.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
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
}
