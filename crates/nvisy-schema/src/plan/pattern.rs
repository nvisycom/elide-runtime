//! Caller-inlined patterns + dictionaries for the pattern
//! recognizer.
//!
//! Two kinds:
//!
//! - [`CustomPatternRule`] — regex-based rule. One or more
//!   [`CustomPatternVariant`]s share a label plus optional
//!   language / country / context scoping.
//! - [`CustomDictionary`] — literal-term rule. Aho-Corasick over
//!   a closed [`CustomDictionaryTerm`] list; faster and safer
//!   than regex when the vocabulary is known ahead of time.
//!
//! Both mirror their `elide-pattern` counterparts one field at a
//! time; the engine converts each wire value into the elide
//! runtime type at analyzer-compile time.
//!
//! ## ReDoS guardrails
//!
//! Custom rules are executable code inside the analyzer, so the
//! wire is bounded:
//!
//! - [`MAX_REGEX_SOURCE_LEN`] caps `CustomPatternVariant.regex`
//!   at deserialize.
//! - Analyzer compile caps the per-request rule count across
//!   `custom` + `custom_dictionaries` combined.
//! - Dictionaries share aggregate term-count and term-byte
//!   limits enforced inside elide's `PatternRecognizerBuilder`.
//!
//! An automaton byte-size budget on regex compile is still an
//! open follow-up — elide's single-budget API cannot separate
//! the per-regex NFA size from the shared `RegexSet` union that
//! the shipped builtins live in. See #317.

use std::collections::HashMap;

use elide_core::entity::LabelRef;
use elide_core::primitive::{Confidence, CountryCode, LanguageTag};
use schemars::JsonSchema;
use serde::{Deserialize, Deserializer, Serialize};

/// Maximum accepted length, in bytes, of one regex source string.
///
/// Rejects at deserialize. Long regex sources are the strongest
/// wire-side signal for a catastrophic-backtracking payload; this
/// cap keeps the ReDoS surface small without touching every regex.
pub const MAX_REGEX_SOURCE_LEN: usize = 512;

/// One caller-supplied regex rule for the pattern recognizer.
///
/// Mirrors `elide_pattern::Regex`. Serialize + Deserialize +
/// JsonSchema end-to-end so SDK callers can inline rules on the
/// wire; the engine converts each rule to an elide `Regex` at
/// analyzer-compile time.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CustomPatternRule {
    /// Human-readable identifier surfaced in provenance.
    pub name: String,
    /// Entity label every variant emits.
    pub label: LabelRef,
    /// Regex sources + per-variant confidence + optional
    /// validator.
    ///
    /// At least one required; the recognizer skips rules with an
    /// empty variant list at compile time.
    pub variants: Vec<CustomPatternVariant>,
    /// Context keywords lifting confidence when they appear near
    /// a match.
    ///
    /// Either a flat list (any language) or a per-language map.
    /// Consumed only when the analyzer's
    /// [`PatternRecognizerParams`]`.context_enhanced` is `true`.
    ///
    /// [`PatternRecognizerParams`]: super::PatternRecognizerParams
    #[serde(default, skip_serializing_if = "CustomPatternContext::is_empty")]
    pub context: CustomPatternContext,
    /// BCP-47 language tags scoping the rule.
    ///
    /// Empty means "any language"; otherwise the recognizer skips
    /// the rule when the per-call language hint is not in the
    /// list.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub languages: Vec<LanguageTag>,
    /// ISO 3166-1 alpha-2 country codes scoping the rule.
    ///
    /// Empty means "any country"; otherwise the recognizer skips
    /// the rule when the per-call jurisdiction hint is not in the
    /// list.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub countries: Vec<CountryCode>,
}

/// One regex variant inside a [`CustomPatternRule`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CustomPatternVariant {
    /// Regex source.
    ///
    /// Capped at [`MAX_REGEX_SOURCE_LEN`] bytes; longer sources
    /// reject at deserialize. Compiled by the engine at
    /// analyzer-compile time.
    #[serde(deserialize_with = "deserialize_bounded_regex")]
    pub regex: String,
    /// Confidence stamped on every match, before any
    /// post-recognition keyword boost.
    ///
    /// Defaults to [`Confidence::MAX`].
    #[serde(default = "confidence_max")]
    pub score: Confidence,
    /// Optional validator name.
    ///
    /// Resolved against elide's `ValidatorRegistry` at compile
    /// time (e.g. `"ssn"`, `"credit_card"`, `"iban"`). Unknown
    /// names error at compile. `None` means "no validation".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub validator: Option<String>,
}

/// Context keywords for a custom rule.
///
/// Either a flat list applied regardless of language, or a
/// per-language map. Matches the shape of `elide_pattern`'s
/// `Context` — untagged so the wire looks like either
/// `["kw1", "kw2"]` or `{ "en": ["kw1"], "es": ["kw2"] }`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum CustomPatternContext {
    /// One flat keyword list applied regardless of language.
    Global(Vec<String>),
    /// Per-language keyword lists.
    PerLanguage(HashMap<LanguageTag, Vec<String>>),
}

impl Default for CustomPatternContext {
    fn default() -> Self {
        Self::Global(Vec::new())
    }
}

impl CustomPatternContext {
    /// True when no keywords are declared in any scope.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        match self {
            Self::Global(kws) => kws.is_empty(),
            Self::PerLanguage(map) => map.values().all(Vec::is_empty),
        }
    }
}

/// One caller-supplied dictionary for the pattern recognizer.
///
/// Mirrors `elide_pattern::Dictionary`. Literal-term matching via
/// Aho-Corasick; faster and safer than regex for closed sets.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CustomDictionary {
    /// Human-readable identifier surfaced in provenance.
    pub name: String,
    /// Entity label every match emits.
    pub label: LabelRef,
    /// Literal terms + per-term confidence overrides.
    ///
    /// At least one required; the recognizer skips dictionaries
    /// with an empty term list at compile time.
    pub terms: Vec<CustomDictionaryTerm>,
    /// Default confidence stamped on matches when a term has no
    /// per-term override.
    ///
    /// Defaults to [`Confidence::MAX`].
    #[serde(default = "confidence_max")]
    pub score: Confidence,
    /// Context keywords lifting confidence near matches.
    ///
    /// Consumed only when the analyzer's
    /// [`PatternRecognizerParams`]`.context_enhanced` is `true`.
    ///
    /// [`PatternRecognizerParams`]: super::PatternRecognizerParams
    #[serde(default, skip_serializing_if = "CustomPatternContext::is_empty")]
    pub context: CustomPatternContext,
    /// BCP-47 language tags scoping the dictionary.
    ///
    /// Empty means "any language"; otherwise the recognizer skips
    /// the dictionary when the per-call language hint is not in
    /// the list.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub languages: Vec<LanguageTag>,
    /// ISO 3166-1 alpha-2 country codes scoping the dictionary.
    ///
    /// Empty means "any country"; otherwise the recognizer skips
    /// the dictionary when the per-call jurisdiction hint is not
    /// in the list.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub countries: Vec<CountryCode>,
}

/// One term inside a [`CustomDictionary`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CustomDictionaryTerm {
    /// The literal scanned for.
    pub term: String,
    /// Per-term score override.
    ///
    /// `None` falls back to the parent dictionary's `score`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub score: Option<Confidence>,
}

fn confidence_max() -> Confidence {
    Confidence::MAX
}

fn deserialize_bounded_regex<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    use serde::de::Error;
    let regex = String::deserialize(deserializer)?;
    if regex.len() > MAX_REGEX_SOURCE_LEN {
        return Err(D::Error::custom(format!(
            "regex source too long: {} bytes exceeds the {}-byte cap; \
             wire-side ReDoS guardrail — split the pattern or lift the \
             match into a validator",
            regex.len(),
            MAX_REGEX_SOURCE_LEN,
        )));
    }
    Ok(regex)
}
