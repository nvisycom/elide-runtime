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
//! - [`Dictionary::from_json`] — self-contained JSON
//!
//! Term sources are first-class — see [`Terms`](crate::Terms) for
//! [`from_text`](crate::Terms::from_text) and
//! [`from_csv`](crate::Terms::from_csv) constructors. The builder's
//! [`with_terms`](DictionaryBuilder::with_terms) setter accepts
//! anything convertible to [`Terms`].
//!
//! [`Pattern`]: crate::Pattern

use derive_builder::Builder;
use nvisy_core::Error;
use nvisy_ontology::entity::EntityKind;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::context::Context;
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
#[serde(rename_all = "camelCase")]
pub struct Dictionary {
    /// Human-readable identifier (e.g. `"nationalities"`).
    pub name: String,
    /// Entity kind every match emits.
    pub entity_kind: EntityKind,
    /// Literal terms to scan for. The recognizer compiles these into
    /// an Aho-Corasick automaton at build time.
    pub terms: Terms,
    /// Confidence score stamped on every match before any boost.
    #[builder(default = "1.0")]
    pub score: f64,
    /// Optional context keywords carried through to emitted entities
    /// for a downstream enhancer to apply boosts.
    #[builder(default)]
    #[serde(default, skip_serializing_if = "context_is_default")]
    pub context: Context,
}

impl Dictionary {
    /// Start a chainable builder. Required fields: `name`,
    /// `entity_kind`, `terms`.
    #[must_use]
    pub fn builder() -> DictionaryBuilder {
        DictionaryBuilder::default()
    }

    /// Parse a self-contained dictionary from a JSON byte slice. The
    /// JSON must include a `terms` field; for metadata-only JSON
    /// paired with a separate term source, use
    /// [`metadata_from_json`](Self::metadata_from_json) instead.
    ///
    /// # Errors
    ///
    /// Returns a validation error when the JSON is malformed or
    /// missing required fields.
    pub fn from_json(bytes: &[u8]) -> Result<Self, Error> {
        serde_json::from_slice(bytes)
            .map_err(|e| Error::validation(format!("dictionary JSON: {e}"), "nvisy-pattern"))
    }

    /// Parse the metadata fields of a dictionary from JSON (no
    /// `terms` required) and return a seeded builder. The caller is
    /// expected to chain
    /// [`with_terms`](DictionaryBuilder::with_terms) before
    /// [`build`](DictionaryBuilder::build).
    ///
    /// Useful when shipped or user-supplied dictionaries split
    /// metadata into a JSON sidecar and store the actual terms as
    /// CSV / TXT.
    ///
    /// # Errors
    ///
    /// Returns a validation error when the JSON is malformed or
    /// missing required metadata fields.
    pub fn metadata_from_json(bytes: &[u8]) -> Result<DictionaryBuilder, Error> {
        let metadata: DictionaryMetadata = serde_json::from_slice(bytes).map_err(|e| {
            Error::validation(format!("dictionary metadata JSON: {e}"), "nvisy-pattern")
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
        Ok(builder)
    }
}

/// Wire shape for the dictionary metadata sidecar JSON — every
/// field [`Dictionary`] carries except `terms`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DictionaryMetadata {
    name: String,
    entity_kind: EntityKind,
    #[serde(default)]
    score: Option<f64>,
    #[serde(default)]
    context: Option<Context>,
}

fn context_is_default(ctx: &Context) -> bool {
    ctx.keywords.is_empty() && ctx.window.is_none() && ctx.boost.is_none()
}
