//! JSON-backed `JsonPattern` implementation.
//!
//! Each JSON file under `assets/patterns/` is deserialized into a
//! `JsonPattern` via `from_bytes`.  The method returns the validated
//! pattern together with any non-fatal `JsonPatternWarning`s so the
//! caller can decide how to surface them.

use serde::Deserialize;

use nvisy_core::data::{EntityCategory, EntityKind};

use super::context_rule::ContextRule;
use super::pattern::{MatchSource, Pattern};

/// Error returned when a JSON pattern file cannot be loaded.
#[derive(Debug, thiserror::Error)]
pub enum JsonPatternError {
    /// The raw bytes are not valid JSON or do not match the expected schema.
    #[error("JSON parse error: {0}")]
    Parse(#[from] serde_json::Error),

    /// Neither `"pattern"` nor `"dictionary"` was provided.
    #[error("pattern '{name}': must specify either 'pattern' or 'dictionary'")]
    MissingSource { name: String },

    /// Both `"pattern"` and `"dictionary"` were provided: only one is allowed.
    #[error("pattern '{name}': cannot specify both 'pattern' and 'dictionary'")]
    AmbiguousSource { name: String },
}

/// Non-fatal warning emitted while loading a pattern.
///
/// Warnings do not prevent the pattern from being registered but may
/// indicate misconfiguration (e.g. a typo in the validator name).
#[derive(Debug)]
pub enum JsonPatternWarning {
    /// The `"category"` value was not a recognised variant and fell through
    /// to [`EntityCategory::Custom`].
    UnknownCategory { pattern: String, slug: String },

    /// The `"validator"` name does not match any built-in validator, so
    /// the pattern will have no post-match validation.
    UnknownValidator { pattern: String, validator: String },
}

/// Default confidence score when `"confidence"` is omitted from JSON.
const DEFAULT_CONFIDENCE: f64 = 1.0;

/// A detection pattern deserialized from a JSON definition file.
///
/// Implements the [`Pattern`] trait and is the only concrete implementation
/// shipped with this crate.  Construct via `from_bytes`.
#[derive(Debug, Clone)]
pub struct JsonPattern {
    name: String,
    category: EntityCategory,
    entity_kind: EntityKind,
    match_source: MatchSource,
    confidence: f64,
    validator: Option<String>,
    case_sensitive: bool,
    pub(crate) context: Option<ContextRule>,
}

impl JsonPattern {
    /// Deserialize and validate a pattern from raw JSON bytes.
    ///
    /// On success returns the pattern together with a (possibly empty)
    /// list of [`JsonPatternWarning`]s.
    ///
    /// # Errors
    ///
    /// Returns [`JsonPatternError`] if the bytes cannot be parsed as JSON,
    /// or if the `"pattern"` / `"dictionary"` fields are missing or
    /// ambiguous.
    pub(crate) fn from_bytes(
        bytes: &[u8],
    ) -> Result<(Self, Vec<JsonPatternWarning>), JsonPatternError> {
        /// Intermediate serde target that mirrors the on-disk JSON shape.
        #[derive(Deserialize)]
        struct Raw {
            name: String,
            category: EntityCategory,
            #[serde(rename = "entity_type")]
            entity_kind: EntityKind,
            #[serde(default)]
            pattern: Option<String>,
            #[serde(default)]
            dictionary: Option<String>,
            #[serde(default)]
            confidence: Option<f64>,
            #[serde(default)]
            validator: Option<String>,
            #[serde(default)]
            case_sensitive: bool,
            #[serde(default)]
            context: Option<ContextRule>,
        }

        let raw: Raw = serde_json::from_slice(bytes)?;

        let match_source = match (raw.pattern, raw.dictionary) {
            (Some(re), None) => MatchSource::Regex(re),
            (None, Some(dict)) => MatchSource::Dictionary(dict),
            (None, None) => return Err(JsonPatternError::MissingSource { name: raw.name }),
            (Some(_), Some(_)) => return Err(JsonPatternError::AmbiguousSource { name: raw.name }),
        };

        let mut warnings = Vec::new();

        if let EntityCategory::Custom(ref slug) = raw.category {
            warnings.push(JsonPatternWarning::UnknownCategory {
                pattern: raw.name.clone(),
                slug: slug.clone(),
            });
        }
        if let Some(ref v) = raw.validator {
            if crate::validators::ValidatorResolver::builtins().resolve(v).is_none() {
                warnings.push(JsonPatternWarning::UnknownValidator {
                    pattern: raw.name.clone(),
                    validator: v.clone(),
                });
            }
        }

        let p = Self {
            name: raw.name,
            category: raw.category,
            entity_kind: raw.entity_kind,
            match_source,
            confidence: raw.confidence.unwrap_or(DEFAULT_CONFIDENCE),
            validator: raw.validator,
            case_sensitive: raw.case_sensitive,
            context: raw.context,
        };

        Ok((p, warnings))
    }
}

impl Pattern for JsonPattern {
    fn name(&self) -> &str {
        &self.name
    }

    fn category(&self) -> &EntityCategory {
        &self.category
    }

    fn entity_kind(&self) -> EntityKind {
        self.entity_kind
    }

    fn match_source(&self) -> &MatchSource {
        &self.match_source
    }

    fn confidence(&self) -> f64 {
        self.confidence
    }

    fn validator_name(&self) -> Option<&str> {
        self.validator.as_deref()
    }

    fn case_sensitive(&self) -> bool {
        self.case_sensitive
    }

    fn context(&self) -> Option<&ContextRule> {
        self.context.as_ref()
    }
}
