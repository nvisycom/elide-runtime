//! [`Regex`]: regex-backed detection rule.
//!
//! A regex rule bundles a regular expression with the entity kind
//! it detects, an emission confidence score, optional context
//! keywords that downstream enhancers can boost on, and an optional
//! named validator (Luhn, IBAN, …) the recognizer runs over each
//! match before emitting an entity.
//!
//! Construct via [`Regex::builder`] for the chainable style or
//! [`Regex::from_toml`] when loading a definition file.

use derive_builder::Builder;
use nvisy_core::Error;
use nvisy_core::context::Context;
use nvisy_core::entity::EntityKind;
use nvisy_core::primitive::{Confidence, LanguageTag};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Regex-backed detection rule.
///
/// Identical fields whether built via [`RegexBuilder`] or loaded
/// from a TOML file via [`Regex::from_toml`].
#[derive(Debug, Clone, PartialEq, Builder)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[builder(
    name = "RegexBuilder",
    pattern = "owned",
    setter(into, strip_option, prefix = "with"),
    build_fn(error = "Error", validate = "RegexBuilder::validate")
)]
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
    #[builder(default = "Confidence::MAX")]
    pub score: Confidence,
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
    /// Languages the rule applies to (BCP-47 tags). An empty list
    /// (the default) means the rule applies regardless of language;
    /// otherwise the recognizer skips this rule when the per-call
    /// language hint is set to a tag not in this list.
    #[builder(default)]
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[schemars(with = "Vec<String>")]
    pub languages: Vec<LanguageTag>,
}

impl Regex {
    /// Start a chainable builder. Required fields: `name`,
    /// `entity_kind`, `regex`.
    #[must_use]
    pub fn builder() -> RegexBuilder {
        RegexBuilder::default()
    }

    /// Parse a regex rule from a TOML string.
    ///
    /// # Errors
    ///
    /// Returns a validation error when the TOML is malformed or
    /// missing required fields.
    pub fn from_toml(raw: &str) -> Result<Self, Error> {
        toml::from_str(raw)
            .map_err(|e| Error::validation(format!("regex TOML: {e}"), "nvisy-pattern"))
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
