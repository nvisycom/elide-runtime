//! Core [`Pattern`] trait and [`MatchSource`] enum.

use std::borrow::Cow;

use globset::{Glob, GlobSetBuilder};
use nvisy_ontology::entity::{EntityCategory, EntityKind};
use regex::Regex;
use serde::{Deserialize, Serialize};

use super::context_rule::ContextRule;
use super::pattern_metadata::PatternMetadata;
use crate::dictionaries::{Dictionary, DictionaryCompile};
use crate::engine::PatternEngineError;
use crate::engine::scan::entries::{CompiledPattern, DictEntry, GlobEntry, RegexEntry};

/// A regex-based match source with an optional post-match validator.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RegexPattern {
    /// The regular expression string.
    pub regex: String,
    /// Optional validator name (e.g. `"luhn"`, `"ssn"`, `"iban"`):
    /// resolved at detection time by the built-in validator
    /// registry.
    #[serde(default)]
    pub validator: Option<String>,
    /// Whether the regex is case-sensitive.
    ///
    /// Defaults to `true`. When `false`, the regex is compiled with
    /// an inline `(?i)` prefix.
    #[serde(default = "default_case_sensitive")]
    pub case_sensitive: bool,
    /// Confidence score (0.0–1.0) assigned to matches from this pattern.
    ///
    /// Defaults to `1.0` when not specified.
    #[serde(default = "default_confidence")]
    pub confidence: f64,
}

impl RegexPattern {
    /// Return the regex string ready for compilation.
    ///
    /// Prepends `(?i)` when [`case_sensitive`] is `false`.
    ///
    /// [`case_sensitive`]: Self::case_sensitive
    pub fn effective_regex(&self) -> String {
        if self.case_sensitive {
            self.regex.clone()
        } else {
            format!("(?i){}", self.regex)
        }
    }
}

/// Confidence for a dictionary pattern: either a single uniform score
/// or per-column scores for CSV dictionaries.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
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
            Self::PerColumn(cols) => cols
                .get(column)
                .copied()
                .unwrap_or_else(|| cols.last().copied().unwrap_or(DEFAULT_CONFIDENCE)),
        }
    }
}

impl Default for DictionaryConfidence {
    fn default() -> Self {
        Self::Uniform(DEFAULT_CONFIDENCE)
    }
}

/// Serde helper: accepts either a single number or an array of numbers.
mod confidence_serde {
    use serde::{Deserialize, Deserializer};

    use super::DictionaryConfidence;

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

/// A glob-based match source.
///
/// Matched token-by-token: input text is split on Unicode word
/// boundaries and each token is checked against the compiled
/// [`globset::GlobSet`]. Use this for shapes that are easier to
/// express as a glob than a regex (`INV-*`, `cust_[0-9][0-9][0-9]`)
/// without escaping regex metacharacters. For substring or
/// cross-token matches use [`RegexPattern`] instead.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GlobPattern {
    /// The glob expression (gitignore dialect: `*`, `?`, `[...]`,
    /// `**`).
    pub glob: String,
    /// Whether matching is case-sensitive.
    ///
    /// Defaults to `true`. When `false`, both the glob and each
    /// token are lower-cased before matching.
    #[serde(default = "default_case_sensitive")]
    pub case_sensitive: bool,
    /// Confidence score (0.0–1.0) assigned to matches from this
    /// pattern. Defaults to `1.0` when not specified.
    #[serde(default = "default_confidence")]
    pub confidence: f64,
}

/// A dictionary-based match source.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DictionaryPattern {
    /// Named dictionary loaded from the built-in dictionary
    /// registry under `assets/dictionaries/`.
    pub name: String,
    /// Whether matching is case-sensitive.
    ///
    /// Defaults to `false`. Controls the Aho-Corasick automaton's
    /// `ascii_case_insensitive` setting.
    #[serde(default)]
    pub case_sensitive: bool,
    /// Confidence score(s) for matches from this dictionary.
    ///
    /// A single number applies uniformly to all entries. An array
    /// assigns per-column confidence for CSV dictionaries (e.g.
    /// `[0.85, 0.55]` gives column 0 entries 0.85 and column 1
    /// entries 0.55).
    ///
    /// Defaults to `1.0` when not specified.
    #[serde(default, deserialize_with = "confidence_serde::deserialize")]
    pub confidence: DictionaryConfidence,
}

/// How a pattern finds matches in text.
///
/// Each pattern uses exactly one source: a regular expression compiled
/// and run against the whole text, a glob matched per-token, or a
/// named dictionary whose entries are matched literally.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MatchSource {
    /// Match via a compiled regular expression.
    Regex(RegexPattern),
    /// Match via a glob expression evaluated per token.
    Glob(GlobPattern),
    /// Match via a named dictionary from the built-in dictionary
    /// registry.
    Dictionary(DictionaryPattern),
}

/// Default confidence score when `"confidence"` is omitted from JSON.
pub const DEFAULT_CONFIDENCE: f64 = 1.0;

fn default_confidence() -> f64 {
    DEFAULT_CONFIDENCE
}

fn default_case_sensitive() -> bool {
    true
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
/// This trait is **sealed**: external crates cannot add new implementations.
/// New pattern sources should be added via JSON files loaded through
/// [`PatternRegistry::load_dir`] or [`PatternRegistry::load_file`].
///
/// [`JsonPattern`]: super::JsonPattern
/// [`PatternRegistry::load_dir`]: super::PatternRegistry::load_dir
/// [`PatternRegistry::load_file`]: super::PatternRegistry::load_file
pub trait Pattern: sealed::Sealed + Send + Sync {
    /// Unique name identifying this pattern (e.g. `"ssn"`, `"credit-card"`).
    fn name(&self) -> &str;

    /// High-level entity category (PersonalIdentity, Financial, Credentials, ...).
    fn category(&self) -> EntityCategory;

    /// Specific entity kind within the category (e.g. `GovernmentId`, `PaymentCard`).
    fn entity_kind(&self) -> EntityKind;

    /// How this pattern matches text: regex or dictionary lookup.
    ///
    /// Confidence scores are embedded in the match source itself:
    /// `RegexPattern::confidence` for regex, `DictionaryPattern::confidence`
    /// for dictionaries.
    fn match_source(&self) -> &MatchSource;

    /// Optional co-occurrence context rule for span-level confidence boosting.
    fn context(&self) -> Option<&ContextRule> {
        None
    }

    /// Optional metadata declared on the pattern (language/industry/
    /// region/compliance tags, version, description, references).
    ///
    /// Returns a reference into the pattern's own storage; the default
    /// returns an empty metadata value, used when no `metadata` block
    /// is present.
    fn metadata(&self) -> &PatternMetadata {
        static EMPTY: std::sync::LazyLock<PatternMetadata> =
            std::sync::LazyLock::new(PatternMetadata::default);
        &EMPTY
    }
}

/// Engine-internal extension that compiles a [`Pattern`] into the
/// runtime entry the engine scans against.
///
/// Lives behind a crate-private trait so the public [`Pattern`]
/// surface stays unaware of the engine-internal [`CompiledPattern`]
/// type. Has a blanket impl over every `Pattern`; consumers only
/// call this from inside the engine builder.
pub(crate) trait PatternCompile {
    /// Compile this pattern into the engine entry it'll be scanned
    /// from. Regex patterns compile their regex; dictionary
    /// patterns resolve their dictionary via `dict_lookup` and
    /// build the Aho-Corasick automaton via the crate-private
    /// `DictionaryCompile::build_automaton`.
    ///
    /// `dict_lookup` is a closure rather than a `&DictionaryRegistry`
    /// so the engine builder can layer user-supplied dictionary
    /// directories on top of the built-ins without cloning the
    /// built-in registry into an owned one.
    ///
    /// Returns `Ok(None)` when a dictionary-backed pattern resolves
    /// to a dictionary with zero terms — the engine treats that as
    /// "nothing to scan for" rather than constructing a no-op
    /// automaton.
    fn compile_with<'a, F>(
        &self,
        dict_lookup: &F,
    ) -> Result<Option<CompiledPattern>, PatternEngineError>
    where
        F: Fn(&str) -> Option<&'a dyn Dictionary>;
}

impl<P: Pattern + ?Sized> PatternCompile for P {
    fn compile_with<'a, F>(
        &self,
        dict_lookup: &F,
    ) -> Result<Option<CompiledPattern>, PatternEngineError>
    where
        F: Fn(&str) -> Option<&'a dyn Dictionary>,
    {
        match self.match_source() {
            MatchSource::Regex(rp) => {
                let effective = rp.effective_regex();
                let regex =
                    Regex::new(&effective).map_err(|e| PatternEngineError::RegexCompile {
                        name: self.name().to_owned(),
                        source: e,
                    })?;
                let entry = RegexEntry {
                    pattern_name: self.name().to_owned(),
                    category: self.category(),
                    entity_kind: self.entity_kind(),
                    confidence: rp.confidence,
                    validator_name: rp.validator.clone(),
                    regex,
                    context: self.context().cloned(),
                };
                Ok(Some(CompiledPattern::Regex {
                    entry,
                    regex_source: effective,
                }))
            }
            MatchSource::Glob(gp) => {
                let effective = if gp.case_sensitive {
                    Cow::Borrowed(gp.glob.as_str())
                } else {
                    Cow::Owned(gp.glob.to_lowercase())
                };
                let mut builder = GlobSetBuilder::new();
                builder.add(Glob::new(effective.as_ref()).map_err(|e| {
                    PatternEngineError::GlobCompile {
                        name: self.name().to_owned(),
                        source: e,
                    }
                })?);
                let set = builder
                    .build()
                    .map_err(|e| PatternEngineError::GlobCompile {
                        name: self.name().to_owned(),
                        source: e,
                    })?;
                Ok(Some(CompiledPattern::Glob(GlobEntry {
                    pattern_name: self.name().to_owned(),
                    category: self.category(),
                    entity_kind: self.entity_kind(),
                    confidence: gp.confidence,
                    case_sensitive: gp.case_sensitive,
                    set,
                    context: self.context().cloned(),
                })))
            }
            MatchSource::Dictionary(dp) => {
                let dict =
                    dict_lookup(&dp.name).ok_or_else(|| PatternEngineError::UnknownDictionary {
                        name: self.name().to_owned(),
                        dictionary: dp.name.clone(),
                    })?;
                let terms: Vec<_> = dict.terms().to_vec();
                if terms.is_empty() {
                    return Ok(None);
                }
                let automaton = dict.build_automaton(dp.case_sensitive).map_err(|e| {
                    PatternEngineError::AhoCorasickBuild {
                        name: self.name().to_owned(),
                        source: e,
                    }
                })?;
                Ok(Some(CompiledPattern::Dictionary(DictEntry {
                    pattern_name: self.name().to_owned(),
                    category: self.category(),
                    entity_kind: self.entity_kind(),
                    confidence: dp.confidence.clone(),
                    automaton,
                    terms,
                    context: self.context().cloned(),
                })))
            }
        }
    }
}

pub(crate) mod sealed {
    pub trait Sealed {}
    impl Sealed for super::super::json_pattern::JsonPattern {}
    impl Sealed for super::super::runtime_pattern::RuntimePattern {}
}
