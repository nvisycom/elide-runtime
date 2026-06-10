//! [`Dictionary`]: literal-term detection rule.
//!
//! A dictionary scans for a fixed list of literal strings using an
//! Aho-Corasick automaton. Compared with [`Pattern`], a dictionary
//! has no regex engine, no validator, and a single shared confidence
//! score applied to every match.
//!
//! Construct via:
//!
//! - [`Dictionary::builder`] — chainable, ground-up
//! - [`Dictionary::from_toml`] — self-contained TOML
//!
//! Term sources are first-class — see [`Terms`] for
//! [`from_text`] and
//! [`from_csv`] constructors. The builder's
//! [`with_terms`] setter accepts
//! anything convertible to [`Terms`].
//!
//! [`Pattern`]: crate::Pattern
//! [`Terms`]: crate::Terms
//! [`from_text`]: crate::Terms::from_text
//! [`from_csv`]: crate::Terms::from_csv
//! [`with_terms`]: DictionaryBuilder::with_terms

use derive_builder::Builder;
use nvisy_core::Error;
use nvisy_core::context::Context;
use nvisy_core::entity::EntityKind;
use nvisy_core::primitive::{Confidence, LanguageTag};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::terms::Terms;

/// Literal-term detection rule.
#[derive(Debug, Clone, PartialEq, Builder)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[builder(
    name = "DictionaryBuilder",
    pattern = "owned",
    setter(into, strip_option, prefix = "with"),
    build_fn(error = "Error")
)]
pub struct Dictionary {
    /// Human-readable identifier (e.g. `"nationalities"`).
    pub name: String,
    /// Entity kind every match emits.
    pub entity_kind: EntityKind,
    /// Literal terms to scan for. The recognizer compiles these into
    /// an Aho-Corasick automaton at build time.
    pub terms: Terms,
    /// Confidence score stamped on every match before any boost.
    #[builder(default = "Confidence::MAX")]
    pub score: Confidence,
    /// Optional context keywords carried through to emitted entities
    /// for a downstream enhancer to apply boosts.
    #[builder(default)]
    #[serde(default, skip_serializing_if = "context_is_default")]
    pub context: Context,
    /// Languages the dictionary applies to (BCP-47 tags). An empty
    /// list (the default) means the dictionary applies regardless of
    /// language; otherwise the recognizer skips this dictionary when
    /// the per-call language hint is set to a tag not in this list.
    #[builder(default)]
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[schemars(with = "Vec<String>")]
    pub languages: Vec<LanguageTag>,
    /// Require word-boundary surroundings on every match. With the
    /// default of `true`, a term `"am"` matches the word `"am"` but
    /// not the `"am"` inside `"example"`. Word characters are
    /// alphanumerics and `_` (Unicode-aware). Set to `false` for
    /// dictionaries that genuinely want substring matching (e.g.
    /// scanning for embedded credentials inside arbitrary tokens).
    #[builder(default = "true")]
    #[serde(default = "default_word_boundary")]
    pub word_boundary: bool,
    /// Per-column confidence overrides for terms loaded from a
    /// multi-column CSV. `column_scores[i]` is the confidence
    /// stamped on every term whose source column was `i`; terms
    /// from a column past the end of this vec fall back to the
    /// dictionary's default `score`. Useful when one column
    /// carries unambiguous long-form names (`English`, `Spanish`)
    /// and another carries short codes (`en`, `es`) that collide
    /// with common words.
    ///
    /// Empty (the default) means "use `score` for every match",
    /// preserving the historical behaviour of single-confidence
    /// dictionaries.
    #[builder(default)]
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub column_scores: Vec<Confidence>,
}

fn default_word_boundary() -> bool {
    true
}

impl Dictionary {
    /// Start a chainable builder. Required fields: `name`,
    /// `entity_kind`, `terms`.
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
            .with_entity_kind(metadata.entity_kind);
        if let Some(score) = metadata.score {
            builder = builder.with_score(score);
        }
        if let Some(context) = metadata.context {
            builder = builder.with_context(context);
        }
        if let Some(wb) = metadata.word_boundary {
            builder = builder.with_word_boundary(wb);
        }
        if let Some(cs) = metadata.column_scores {
            builder = builder.with_column_scores(cs);
        }
        Ok(builder)
    }
}

/// Wire shape for the dictionary metadata sidecar TOML — every
/// field [`Dictionary`] carries except `terms`.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct DictionaryMetadata {
    name: String,
    entity_kind: EntityKind,
    #[serde(default)]
    score: Option<Confidence>,
    #[serde(default)]
    context: Option<Context>,
    #[serde(default)]
    word_boundary: Option<bool>,
    #[serde(default)]
    column_scores: Option<Vec<Confidence>>,
}

fn context_is_default(ctx: &Context) -> bool {
    ctx.is_empty() && ctx.window.is_none() && ctx.boost.is_none()
}
