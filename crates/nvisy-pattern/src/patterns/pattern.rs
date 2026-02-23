//! Core [`Pattern`] trait, [`MatchSource`] enum, and [`BoxPattern`] alias.
//!
//! [`Pattern`]: crate::patterns::Pattern
//! [`MatchSource`]: crate::patterns::MatchSource
//! [`BoxPattern`]: crate::patterns::BoxPattern

use serde::Deserialize;

use nvisy_core::data::{EntityCategory, EntityKind};

use super::context_rule::ContextRule;

/// A regex-based match source with an optional post-match validator.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct RegexPattern {
    /// The regular expression string.
    pub regex: String,
    /// Optional validator name (e.g. `"luhn"`, `"ssn"`, `"iban"`),
    /// resolved at detection time via [`ValidatorResolver`].
    ///
    /// [`ValidatorResolver`]: crate::validators::ValidatorResolver
    #[serde(default)]
    pub validator: Option<String>,
    /// Whether the regex is case-sensitive.
    ///
    /// Defaults to `false`.  When `false`, the regex is compiled with
    /// inline `(?i)` or equivalent flag.
    #[serde(default)]
    pub case_sensitive: bool,
}

/// A dictionary-based match source.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct DictionaryPattern {
    /// Named dictionary from the [`DictionaryRegistry`].
    ///
    /// [`DictionaryRegistry`]: crate::dictionaries::DictionaryRegistry
    pub name: String,
    /// Whether matching is case-sensitive.
    ///
    /// Defaults to `false`.  Controls the Aho-Corasick automaton's
    /// `ascii_case_insensitive` setting.
    #[serde(default)]
    pub case_sensitive: bool,
}

/// How a pattern finds matches in text.
///
/// Each pattern uses exactly one source: either a regular expression that
/// is compiled and run against text spans, or a named dictionary whose
/// entries are matched literally.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MatchSource {
    /// Match via a compiled regular expression.
    Regex(RegexPattern),
    /// Match via a named dictionary from the [`DictionaryRegistry`].
    ///
    /// [`DictionaryRegistry`]: crate::dictionaries::DictionaryRegistry
    Dictionary(DictionaryPattern),
}

/// A named detection pattern.
///
/// Implementors describe a single entity type to detect, including how to
/// match it ([`MatchSource`]), how to classify it ([`EntityCategory`] and
/// [`EntityKind`]), how confident the match is, and whether an optional
/// post-match validator should be applied.
///
/// The built-in implementation is [`JsonPattern`], which is deserialized
/// from the JSON files under `assets/patterns/`.
///
/// [`JsonPattern`]: super::JsonPattern
pub trait Pattern: Send + Sync {
    /// Unique name identifying this pattern (e.g. `"ssn"`, `"credit-card"`).
    fn name(&self) -> &str;

    /// High-level entity category (PII, Financial, Credentials, ...).
    fn category(&self) -> &EntityCategory;

    /// Specific entity kind within the category (e.g. `GovernmentId`, `PaymentCard`).
    fn entity_kind(&self) -> EntityKind;

    /// How this pattern matches text: regex or dictionary lookup.
    ///
    /// For regex patterns, the validator (if any) is embedded in the
    /// [`MatchSource::Regex`] variant.
    fn match_source(&self) -> &MatchSource;

    /// Base confidence score (0.0–1.0) assigned to every raw match.
    ///
    /// Defaults to `1.0` when not specified in the pattern definition.
    fn confidence(&self) -> f64;

    /// Optional co-occurrence context rule for span-level confidence boosting.
    fn context(&self) -> Option<&ContextRule> {
        None
    }
}

/// Type-erased boxed [`Pattern`].
pub type BoxPattern = Box<dyn Pattern>;
