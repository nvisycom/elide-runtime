//! [`Regex`]: regex-backed detection rule.
//!
//! A regex rule bundles a regular expression with the entity kind
//! it detects, an emission confidence score, optional context
//! keywords that downstream enhancers can boost on, and an optional
//! named validator (Luhn, IBAN, …) the recognizer runs over each
//! match before emitting an entity.
//!
//! Construct via [`Regex::builder`] for the chainable style or
//! [`Regex::from_json`] when loading a definition file.

use derive_builder::Builder;
use nvisy_core::Error;
use nvisy_core::context::Context;
use nvisy_ontology::entity::EntityKind;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Regex-backed detection rule.
///
/// Identical fields whether built via [`RegexBuilder`] or loaded
/// from a JSON file via [`Regex::from_json`].
#[derive(Debug, Clone, PartialEq, Builder)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[builder(
    name = "RegexBuilder",
    pattern = "owned",
    setter(into, strip_option, prefix = "with"),
    build_fn(error = "Error", validate = "RegexBuilder::validate")
)]
#[serde(rename_all = "camelCase")]
pub struct Regex {
    /// Human-readable identifier (e.g. `"ssn"`, `"credit_card"`).
    /// Surfaced in trail steps so downstream consumers can see
    /// which rule matched.
    pub name: String,
    /// Entity kind every match emits.
    pub entity_kind: EntityKind,
    /// Regex source. Compiled to a [`regex::Regex`] by
    /// [`PatternRecognizer::build`]; shape
    /// errors there, not here.
    ///
    /// [`PatternRecognizer::build`]: super::PatternRecognizer
    pub regex: String,
    /// Confidence score stamped on every match before any boost.
    #[builder(default = "1.0")]
    pub score: f64,
    /// Optional context keywords. Carried through to emitted
    /// entities so a downstream enhancer can apply boosts.
    #[builder(default)]
    #[serde(default, skip_serializing_if = "context_is_default")]
    pub context: Context,
    /// Optional validator name. Resolved at recognizer build time
    /// against the [`ValidatorRegistry`].
    /// Matches that fail validation are dropped.
    ///
    /// [`ValidatorRegistry`]: crate::validators::ValidatorRegistry
    #[builder(default)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub validator: Option<String>,
}

impl Regex {
    /// Start a chainable builder. Required fields: `name`,
    /// `entity_kind`, `regex`.
    #[must_use]
    pub fn builder() -> RegexBuilder {
        RegexBuilder::default()
    }

    /// Parse a regex rule from a JSON byte slice.
    ///
    /// # Errors
    ///
    /// Returns a validation error when the JSON is malformed or
    /// missing required fields.
    pub fn from_json(bytes: &[u8]) -> Result<Self, Error> {
        serde_json::from_slice(bytes)
            .map_err(|e| Error::validation(format!("regex JSON: {e}"), "nvisy-pattern"))
    }
}

impl RegexBuilder {
    fn validate(&self) -> Result<(), Error> {
        if let Some(regex) = self.regex.as_ref()
            && let Err(e) = ::regex::Regex::new(regex)
        {
            return Err(Error::validation(
                format!("invalid regex: {e}"),
                "nvisy-pattern",
            ));
        }
        Ok(())
    }
}

fn context_is_default(ctx: &Context) -> bool {
    ctx.is_empty() && ctx.window.is_none() && ctx.boost.is_none()
}
