//! Core [`Pattern`] trait, [`MatchSource`] enum, and [`BoxPattern`] alias.
//!
//! [`Pattern`]: crate::patterns::Pattern
//! [`MatchSource`]: crate::patterns::MatchSource
//! [`BoxPattern`]: crate::patterns::BoxPattern

use nvisy_core::data::{EntityCategory, EntityKind};

/// How a pattern finds matches in text.
///
/// Each pattern uses exactly one source: either a regular expression that
/// is compiled and run against text spans, or a named dictionary whose
/// entries are matched literally.
///
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MatchSource {
    /// Match via a compiled regular expression.
    Regex(String),
    /// Match via a named dictionary from the [`DictionaryRegistry`].
    ///
    /// [`DictionaryRegistry`]: crate::dictionaries::DictionaryRegistry
    Dictionary(String),
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
    fn match_source(&self) -> &MatchSource;

    /// Base confidence score (0.0–1.0) assigned to every raw match.
    ///
    /// Defaults to `1.0` when not specified in the pattern definition.
    fn confidence(&self) -> f64;

    /// Name of the post-match validator (e.g. `"luhn"`), if any.
    ///
    /// Resolved at detection time via [`ValidatorResolver`].
    ///
    /// [`ValidatorResolver`]: crate::validators::ValidatorResolver
    fn validator_name(&self) -> Option<&str>;

    /// Whether matching should be case-sensitive.
    ///
    /// Defaults to `false` (case-insensitive).  Dictionary-backed
    /// patterns use this to configure the Aho-Corasick automaton;
    /// regex patterns use inline flags instead.
    fn case_sensitive(&self) -> bool {
        false
    }
}

/// Type-erased boxed [`Pattern`].
pub type BoxPattern = Box<dyn Pattern>;
