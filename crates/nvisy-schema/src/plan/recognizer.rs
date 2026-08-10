//! Recognizer params: caller-inlined regex rules and dictionaries
//! for the built-in pattern recognizer.
//!
//! Three recognizer kinds run against every request the engine
//! sees:
//!
//! - **Pattern** — always on. The bare shipped set from
//!   `elide-pattern` (built-in regex + dictionaries, country-
//!   scoped for the request's asserted jurisdictions) plus any
//!   caller-inlined [`CustomPatternRule`] / [`CustomDictionary`]
//!   the request carries. Wrapped in elide's `Enhanced` layer so
//!   per-label context keywords always boost low-confidence
//!   matches.
//! - **NER** — every recognizer wired via `Engine::with_ner`
//!   runs on every request. The deployment picks the lineup;
//!   there is no per-request opt-out.
//! - **LLM** — same shape as NER, filtered further by declared
//!   modality (only recognizers whose `modalities` list contains
//!   the analyzer's modality attach).
//!
//! Rationale for the always-on NER/LLM shape: deployments that
//! don't want an inference recognizer running just don't wire
//! it. Per-request opt-out invited callers to skip work they
//! then didn't realise they had, and callers that genuinely
//! want cheaper analyses can shard traffic to a cheaper
//! deployment.
//!
//! [`AnalyzerParams`]: super::AnalyzerParams

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::pattern::{CustomDictionary, CustomPatternRule};

/// Caller-inlined additions to the built-in pattern recognizer.
///
/// The bare shipped `elide-pattern` set is always attached;
/// these fields carry request-specific extras alongside it.
#[derive(Debug, Clone, Default, PartialEq)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RecognizerParams {
    /// Caller-inlined regex rules compiled per-request.
    ///
    /// See [`CustomPatternRule`] for the shape; the engine bounds
    /// compile cost with the same rule-count cap and per-regex
    /// NFA-size limit that apply to the built-in set, on top of
    /// the deserialize-time source-length cap in
    /// [`MAX_REGEX_SOURCE_LEN`].
    ///
    /// [`MAX_REGEX_SOURCE_LEN`]: super::MAX_REGEX_SOURCE_LEN
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub custom: Vec<CustomPatternRule>,
    /// Caller-inlined literal-term dictionaries compiled
    /// per-request into a shared Aho-Corasick automaton.
    ///
    /// Same rule-count cap as [`custom`] applies at compile time.
    ///
    /// [`custom`]: RecognizerParams::custom
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub custom_dictionaries: Vec<CustomDictionary>,
}
