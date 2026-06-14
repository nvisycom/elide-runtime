//! [`Regex`]: per-label regex-based detection rule.
//!
//! A `Regex` rule bundles one entity label, its context-keyword
//! list, and one or more [`Variant`]s. Each variant carries its
//! own regex source, emission score, and optional named
//! validator. All variants under one rule emit the same label.
//!
//! Construct via [`Regex::builder`] for the chainable style or
//! [`Regex::from_toml`] when loading a definition file.

use derive_builder::Builder;
use nvisy_core::Error;
use nvisy_core::entity::EntityLabelRef;
use nvisy_core::primitive::{Confidence, LanguageTag};
use serde::Deserialize;

/// One regex variant inside a [`Regex`] rule. Carries the regex
/// source, the emission confidence stamped on every match, and the
/// optional validator name resolved at recognizer-build time.
#[derive(Debug, Clone, PartialEq, Builder, Deserialize)]
#[builder(
    name = "VariantBuilder",
    pattern = "owned",
    setter(into, strip_option, prefix = "with"),
    build_fn(error = "Error", validate = "VariantBuilder::validate")
)]
pub struct Variant {
    /// Regex source. Compiled to a [`::regex::Regex`] by
    /// [`PatternRecognizer::build`]; shape errors there, not here.
    ///
    /// [`PatternRecognizer::build`]: super::PatternRecognizer
    pub regex: String,
    /// Confidence score stamped on every match this variant emits
    /// before any post-recognition boost.
    #[builder(default = "Confidence::MAX")]
    pub score: Confidence,
    /// Optional validator name. Resolved at recognizer-build time
    /// against the [`ValidatorRegistry`]; matches that fail
    /// validation are dropped.
    ///
    /// [`ValidatorRegistry`]: crate::validators::ValidatorRegistry
    #[builder(default)]
    #[serde(default)]
    pub validator: Option<String>,
}

impl Variant {
    /// Start a chainable builder. Required field: `regex`.
    #[must_use]
    pub fn builder() -> VariantBuilder {
        VariantBuilder::default()
    }
}

impl VariantBuilder {
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

/// Regex-based detection rule: one label, optional boost
/// keywords, one or more [`Variant`]s. Matches the Presidio
/// "pattern recognizer" shape — multiple regex strategies for one
/// entity type, plus a shared context keyword list.
///
/// ```
/// use nvisy_core::entity::builtins;
/// use nvisy_core::primitive::Confidence;
/// use nvisy_pattern::{Regex, Variant};
///
/// let variant = Variant::builder()
///     .with_regex(r"\b\d{3}-\d{2}-\d{4}\b")
///     .with_score(Confidence::clamped(0.9))
///     .with_validator("ssn")
///     .build()
///     .expect("ssn variant builds");
///
/// let ssn = Regex::builder()
///     .with_name("ssn")
///     .with_label(builtins::GOVERNMENT_ID.label_ref())
///     .with_context(vec!["ssn".to_owned(), "social security".to_owned()])
///     .with_variants(vec![variant])
///     .build()
///     .expect("ssn rule builds");
/// ```
#[derive(Debug, Clone, PartialEq, Builder, Deserialize)]
#[builder(
    name = "RegexBuilder",
    pattern = "owned",
    setter(into, strip_option, prefix = "with"),
    build_fn(error = "Error")
)]
pub struct Regex {
    /// Human-readable identifier (e.g. `"ssn"`, `"credit_card"`).
    /// Surfaced in trail steps so downstream consumers can see
    /// which rule matched.
    pub name: String,
    /// Entity label every variant emits.
    pub label: EntityLabelRef,
    /// Context keywords that lift confidence when one of them
    /// appears near a match. Harvested by [`PatternRecognizer`]
    /// into a per-label boost rule; rules themselves never read
    /// this field.
    ///
    /// [`PatternRecognizer`]: super::PatternRecognizer
    #[builder(default)]
    #[serde(default)]
    pub context: Vec<String>,
    /// Regex variants. At least one is required for the rule to
    /// produce any matches; the recognizer skips rules with no
    /// variants.
    pub variants: Vec<Variant>,
    /// Languages this rule applies to (BCP-47 tags). An empty
    /// list (the default) means the rule applies regardless of
    /// language; otherwise the recognizer skips this rule when
    /// the per-call language hint is set to a tag not in this
    /// list.
    #[builder(default)]
    #[serde(default)]
    pub languages: Vec<LanguageTag>,
}

impl Regex {
    /// Start a chainable builder. Required fields: `name`,
    /// `label`, `variants`.
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
            .map_err(|e| Error::validation(format!("regex rule TOML: {e}"), "nvisy-pattern"))
    }
}
