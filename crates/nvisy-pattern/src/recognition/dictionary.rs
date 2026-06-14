//! [`Dictionary`]: literal-term detection rule.
//!
//! A dictionary scans for a fixed list of literal strings using an
//! Aho-Corasick automaton. Compared with [`Regex`], a dictionary
//! has no regex engine, no validator, and a single shared confidence
//! score applied to every match.
//!
//! Construct via [`Dictionary::builder`] for the chainable style or
//! [`Dictionary::from_toml`] for a self-contained TOML source.
//!
//! Term sources are first-class — see [`Terms`] for [`from_text`]
//! and [`from_csv`] constructors. The builder's [`with_terms`]
//! setter accepts anything convertible to [`Terms`].
//!
//! [`Regex`]: crate::Regex
//! [`Terms`]: crate::Terms
//! [`from_text`]: crate::Terms::from_text
//! [`from_csv`]: crate::Terms::from_csv
//! [`with_terms`]: DictionaryBuilder::with_terms

use derive_builder::Builder;
use nvisy_core::Error;
use nvisy_core::entity::EntityLabelRef;
use nvisy_core::primitive::{Confidence, LanguageTag};
use serde::Deserialize;

use super::terms::Terms;

/// Confidence policy for a [`Dictionary`]'s matches.
///
/// Either every term gets the same score ([`Uniform`]), or scores
/// are picked per CSV source column ([`PerColumn`]). The untagged
/// serde representation accepts a bare number for the uniform
/// case and an array for the per-column case:
///
/// ```toml
/// score = 0.9              # Uniform
/// score = [0.85, 0.30]     # PerColumn
/// ```
///
/// [`Uniform`]: Scoring::Uniform
/// [`PerColumn`]: Scoring::PerColumn
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(untagged)]
pub enum Scoring {
    /// Single confidence stamped on every match. The common case.
    Uniform(Confidence),
    /// Per-column confidence vector. `[i]` is the confidence
    /// stamped on every term whose source CSV column was `i`. A
    /// term from a column past the end of this vec is a
    /// recognizer-build error — define one score per column.
    PerColumn(Vec<Confidence>),
}

impl Scoring {
    /// Validate the policy's internal shape. A
    /// `PerColumn(vec![])` can never resolve a score for any
    /// column, so callers (the recognizer at build time) surface
    /// it as a configuration error.
    ///
    /// # Errors
    ///
    /// Returns the human-readable reason the policy is invalid.
    pub fn validate(&self) -> Result<(), &'static str> {
        match self {
            Self::Uniform(_) => Ok(()),
            Self::PerColumn(scores) if scores.is_empty() => {
                Err("PerColumn scoring with no scores can never resolve")
            }
            Self::PerColumn(_) => Ok(()),
        }
    }

    /// Resolve a score for `column`. `Uniform` ignores the column
    /// and always returns its score; `PerColumn` returns the entry
    /// at `column`, or `None` when no column is supplied or the
    /// index is past the end of the per-column vector. Callers
    /// decide the fall-back policy (per-term override, hard
    /// error, default constant, etc.).
    #[must_use]
    pub fn get(&self, column: Option<u16>) -> Option<Confidence> {
        match self {
            Self::Uniform(s) => Some(*s),
            Self::PerColumn(scores) => column.and_then(|c| scores.get(c as usize).copied()),
        }
    }
}

impl Default for Scoring {
    fn default() -> Self {
        Self::Uniform(Confidence::MAX)
    }
}

/// Literal-term detection rule.
///
/// ```
/// use nvisy_core::entity::builtins;
/// use nvisy_pattern::{Dictionary, Terms};
///
/// let dictionary = Dictionary::builder()
///     .with_name("nationalities")
///     .with_label(builtins::NATIONALITY.label_ref())
///     .with_terms(Terms::from(["German", "French", "Italian"]))
///     .build()
///     .expect("nationalities dictionary builds");
/// ```
#[derive(Debug, Clone, PartialEq, Builder, Deserialize)]
#[builder(
    name = "DictionaryBuilder",
    pattern = "owned",
    setter(into, strip_option, prefix = "with"),
    build_fn(error = "Error")
)]
pub struct Dictionary {
    /// Human-readable identifier (e.g. `"nationalities"`).
    pub name: String,
    /// Entity label every match emits.
    pub label: EntityLabelRef,
    /// Literal terms to scan for. The recognizer compiles these
    /// into an Aho-Corasick automaton at build time.
    pub terms: Terms,
    /// Confidence policy: uniform across every term, or per CSV
    /// source column. Defaults to [`Scoring::Uniform`] with
    /// [`Confidence::MAX`].
    #[builder(default)]
    #[serde(default, rename = "score")]
    pub scoring: Scoring,
    /// Context keywords that lift confidence when one of them
    /// appears near a match. Harvested by the engine into a
    /// per-label `BoostRule` in `nvisy-context`; the recognizer
    /// itself never reads this field.
    #[builder(default)]
    #[serde(default)]
    pub context: Vec<String>,
    /// Languages the dictionary applies to (BCP-47 tags). An empty
    /// list (the default) means the dictionary applies regardless
    /// of language; otherwise the recognizer skips this dictionary
    /// when the per-call language hint is set to a tag not in this
    /// list.
    #[builder(default)]
    #[serde(default)]
    pub languages: Vec<LanguageTag>,
    /// Require word-boundary surroundings on every match. With the
    /// default of `true`, a term `"am"` matches the word `"am"`
    /// but not the `"am"` inside `"example"`. Word characters are
    /// alphanumerics and `_` (Unicode-aware). Set to `false` for
    /// dictionaries that genuinely want substring matching (e.g.
    /// scanning for embedded credentials inside arbitrary tokens).
    #[builder(default = "true")]
    #[serde(default = "default_word_boundary")]
    pub word_boundary: bool,
}

fn default_word_boundary() -> bool {
    true
}

impl Dictionary {
    /// Start a chainable builder. Required fields: `name`,
    /// `label`, `terms`.
    #[must_use]
    pub fn builder() -> DictionaryBuilder {
        DictionaryBuilder::default()
    }

    /// Parse a self-contained dictionary from a TOML string. The
    /// TOML must include a `terms` field; for metadata-only TOML
    /// paired with a separate term source, use
    /// [`metadata_from_toml`] instead.
    ///
    /// # Errors
    ///
    /// Returns a validation error when the TOML is malformed or
    /// missing required fields.
    ///
    /// [`metadata_from_toml`]: Self::metadata_from_toml
    pub fn from_toml(raw: &str) -> Result<Self, Error> {
        toml::from_str(raw)
            .map_err(|e| Error::validation(format!("dictionary TOML: {e}"), "nvisy-pattern"))
    }

    /// Parse the metadata fields of a dictionary from TOML (no
    /// `terms` required) and return a seeded builder. The caller is
    /// expected to chain
    /// [`with_terms`] before
    /// [`build`].
    ///
    /// Useful when shipped or user-supplied dictionaries split
    /// metadata into a TOML sidecar and store the actual terms as
    /// CSV / TXT.
    ///
    /// # Errors
    ///
    /// Returns a validation error when the TOML is malformed or
    /// missing required metadata fields.
    ///
    /// [`with_terms`]: DictionaryBuilder::with_terms
    /// [`build`]: DictionaryBuilder::build
    pub fn metadata_from_toml(raw: &str) -> Result<DictionaryBuilder, Error> {
        let metadata: DictionaryMetadata = toml::from_str(raw).map_err(|e| {
            Error::validation(format!("dictionary metadata TOML: {e}"), "nvisy-pattern")
        })?;
        let mut builder = Dictionary::builder()
            .with_name(metadata.name)
            .with_label(metadata.label);
        if let Some(scoring) = metadata.score {
            builder = builder.with_scoring(scoring);
        }
        if let Some(context) = metadata.context {
            builder = builder.with_context(context);
        }
        if let Some(wb) = metadata.word_boundary {
            builder = builder.with_word_boundary(wb);
        }
        Ok(builder)
    }
}

/// Wire shape for the dictionary metadata sidecar TOML — every
/// field [`Dictionary`] carries except `terms`.
#[derive(Debug, Clone, Deserialize)]
struct DictionaryMetadata {
    name: String,
    label: EntityLabelRef,
    #[serde(default)]
    score: Option<Scoring>,
    #[serde(default)]
    context: Option<Vec<String>>,
    #[serde(default)]
    word_boundary: Option<bool>,
}
