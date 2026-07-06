//! Pattern-recognizer guardrail configuration.
//!
//! Deployment-owned caps that bound the ReDoS attack surface
//! and automaton compile cost when callers inline custom regex
//! rules and dictionaries on
//! `nvisy_schema::plan::PatternRecognizerParams`.
//!
//! Every knob has a conservative default; a deployment can
//! tighten below the default but not raise past a hard ceiling
//! also enforced at the wire layer (see
//! `nvisy_schema::plan::MAX_REGEX_SOURCE_LEN` for the regex
//! source-length ceiling).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Deployment-side caps applied to
/// [`PatternRecognizerParams`]-driven recognizer builds.
///
/// Loaded from the deployment's `[pattern.guardrails]` config
/// section; passed into the engine via
/// [`Engine::with_pattern_guardrails`].
///
/// [`PatternRecognizerParams`]: nvisy_schema::plan::PatternRecognizerParams
/// [`Engine::with_pattern_guardrails`]: crate::Engine::with_pattern_guardrails
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", default)]
pub struct PatternGuardrails {
    /// Maximum accepted length, in bytes, of one custom regex
    /// source.
    ///
    /// Enforced at engine compile time. May tighten below the
    /// wire-layer ceiling of 512 bytes (checked at deserialize
    /// in `nvisy-schema`) but not raise above it. Values above
    /// the ceiling are clamped at engine construction.
    pub max_regex_source_len: usize,
    /// Maximum number of caller-inlined rules per request across
    /// `custom` and `custom_dictionaries` combined.
    ///
    /// Bounds request-level compile cost: elide compiles each
    /// regex and every dictionary term into the recognizer's
    /// automata at analyze time. A permissive cap here would let
    /// a caller pin a worker on compile alone.
    pub max_custom_rules: usize,
    /// Aggregate cap on total dictionary terms across every
    /// dictionary — builtin and custom.
    ///
    /// All dictionaries compile into one shared Aho-Corasick
    /// automaton; this is the total-terms cap, not
    /// per-dictionary.
    pub max_dictionary_term_count: usize,
    /// Aggregate byte budget across every dictionary's terms.
    ///
    /// Finer proxy for the same automaton cost that
    /// [`max_dictionary_term_count`] bounds; both apply, and
    /// whichever hits first fails the build. Dictionaries are
    /// literal-match, so this is a compile-cost bound, not a
    /// match-time hazard.
    ///
    /// [`max_dictionary_term_count`]: PatternGuardrails::max_dictionary_term_count
    pub max_dictionary_term_bytes: usize,
}

impl Default for PatternGuardrails {
    fn default() -> Self {
        Self {
            max_regex_source_len: nvisy_schema::plan::MAX_REGEX_SOURCE_LEN,
            max_custom_rules: 32,
            max_dictionary_term_count: 100_000,
            max_dictionary_term_bytes: 8 * 1024 * 1024,
        }
    }
}

impl PatternGuardrails {
    /// Clamp `max_regex_source_len` to the wire-layer ceiling.
    ///
    /// The schema deserialize check enforces
    /// [`MAX_REGEX_SOURCE_LEN`] at the wire boundary; a
    /// deployment can only tighten below it, never raise above.
    /// [`Engine::with_pattern_guardrails`] applies this before
    /// storing the config so callers can't set a permissive
    /// value that would never actually take effect.
    ///
    /// [`MAX_REGEX_SOURCE_LEN`]: nvisy_schema::plan::MAX_REGEX_SOURCE_LEN
    /// [`Engine::with_pattern_guardrails`]: crate::Engine::with_pattern_guardrails
    #[must_use]
    pub fn clamped(mut self) -> Self {
        self.max_regex_source_len = self
            .max_regex_source_len
            .min(nvisy_schema::plan::MAX_REGEX_SOURCE_LEN);
        self
    }
}
