//! Core [`Pattern`] trait, [`MatchSource`] enum, and [`BoxPattern`] alias.
//!
//! [`Pattern`]: crate::patterns::Pattern
//! [`MatchSource`]: crate::patterns::MatchSource
//! [`BoxPattern`]: crate::patterns::BoxPattern

use serde::Deserialize;

use nvisy_ontology::entity::{EntityCategory, EntityKind};

use super::context_rule::ContextRule;

/// A regex-based match source with an optional post-match validator.
#[derive(Debug, Clone, PartialEq, Deserialize)]
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
    /// Confidence score (0.0–1.0) assigned to matches from this pattern.
    ///
    /// Defaults to `1.0` when not specified.
    #[serde(default = "default_confidence")]
    pub confidence: f64,
}

/// Confidence for a dictionary pattern: either a single uniform score
/// or per-column scores for CSV dictionaries.
#[derive(Debug, Clone, PartialEq)]
pub enum DictionaryConfidence {
    /// Single confidence score applied to all entries.
    Uniform(f64),
    /// Per-column confidence scores. Entries from column `i` use index `i`.
    /// Columns beyond the length fall back to the last value.
    PerColumn(Vec<f64>),
}

impl DictionaryConfidence {
    /// Resolve confidence for a given column index.
    pub fn resolve(&self, column: usize) -> f64 {
        match self {
            Self::Uniform(c) => *c,
            Self::PerColumn(cols) => cols.get(column).copied().unwrap_or_else(|| {
                cols.last().copied().unwrap_or(DEFAULT_CONFIDENCE)
            }),
        }
    }

}

impl Default for DictionaryConfidence {
    fn default() -> Self {
        Self::Uniform(DEFAULT_CONFIDENCE)
    }
}

/// Serde helper — accepts either a single number or an array of numbers.
mod confidence_serde {
    use super::DictionaryConfidence;
    use serde::{Deserialize, Deserializer};

    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Raw {
        Uniform(f64),
        PerColumn(Vec<f64>),
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<DictionaryConfidence, D::Error> {
        Ok(match Raw::deserialize(deserializer)? {
            Raw::Uniform(c) => DictionaryConfidence::Uniform(c),
            Raw::PerColumn(v) => DictionaryConfidence::PerColumn(v),
        })
    }
}

/// A dictionary-based match source.
#[derive(Debug, Clone, PartialEq, Deserialize)]
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
    /// Confidence score(s) for matches from this dictionary.
    ///
    /// A single number applies uniformly to all entries.
    /// An array assigns per-column confidence for CSV dictionaries
    /// (e.g. `[0.85, 0.55]` gives column 0 entries 0.85 and column 1
    /// entries 0.55).
    ///
    /// Defaults to `1.0` when not specified.
    #[serde(default, deserialize_with = "confidence_serde::deserialize")]
    pub confidence: DictionaryConfidence,
}

/// How a pattern finds matches in text.
///
/// Each pattern uses exactly one source: either a regular expression that
/// is compiled and run against text spans, or a named dictionary whose
/// entries are matched literally.
#[derive(Debug, Clone, PartialEq)]
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
/// Default confidence score when `"confidence"` is omitted from JSON.
pub const DEFAULT_CONFIDENCE: f64 = 1.0;

fn default_confidence() -> f64 {
    DEFAULT_CONFIDENCE
}

pub trait Pattern: Send + Sync {
    /// Unique name identifying this pattern (e.g. `"ssn"`, `"credit-card"`).
    fn name(&self) -> &str;

    /// High-level entity category (PII, Financial, Credentials, ...).
    fn category(&self) -> &EntityCategory;

    /// Specific entity kind within the category (e.g. `GovernmentId`, `PaymentCard`).
    fn entity_kind(&self) -> EntityKind;

    /// How this pattern matches text: regex or dictionary lookup.
    ///
    /// Confidence scores are embedded in the match source itself:
    /// [`RegexPattern::confidence`] for regex, [`DictionaryPattern::confidence`]
    /// for dictionaries.
    fn match_source(&self) -> &MatchSource;

    /// Optional co-occurrence context rule for span-level confidence boosting.
    fn context(&self) -> Option<&ContextRule> {
        None
    }
}

/// Type-erased boxed [`Pattern`].
pub type BoxPattern = Box<dyn Pattern>;
