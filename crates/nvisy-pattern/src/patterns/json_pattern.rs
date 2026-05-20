//! JSON-backed [`JsonPattern`] implementation.
//!
//! Each JSON file under `assets/patterns/` is deserialized into a
//! [`JsonPattern`] via [`from_bytes`]. The method
//! returns the validated pattern together with any non-fatal
//! [`JsonPatternWarning`]s so the caller can decide how to surface them.
//!
//! [`from_bytes`]: JsonPattern::from_bytes

use nvisy_ontology::entity::{EntityCategory, EntityKind};
use serde::Deserialize;

use super::context_rule::ContextRule;
use super::pattern::{DictionaryPattern, MatchSource, Pattern, RegexPattern};
use super::pattern_metadata::PatternMetadata;
use crate::validators::ValidatorResolver;

/// Error returned when a JSON pattern file cannot be loaded.
#[derive(Debug, thiserror::Error)]
pub enum JsonPatternError {
    /// The raw bytes are not valid JSON or do not match the expected schema.
    #[error("JSON parse error: {0}")]
    Parse(#[from] serde_json::Error),
}

/// Non-fatal warning emitted while loading a pattern.
///
/// Warnings do not prevent the pattern from being registered but may
/// indicate misconfiguration (e.g. a typo in the validator name).
#[derive(Debug)]
pub enum JsonPatternWarning {
    /// The `"validator"` name does not match any built-in validator, so
    /// the pattern will have no post-match validation.
    UnknownValidator { pattern: String, validator: String },
}

/// A detection pattern deserialized from a JSON definition file.
///
/// Implements the [`Pattern`] trait and is the only concrete implementation
/// shipped with this crate. Construct via `from_bytes`.
#[derive(Debug, Clone)]
pub struct JsonPattern {
    name: String,
    category: EntityCategory,
    entity_kind: EntityKind,
    match_source: MatchSource,
    pub(crate) context: Option<ContextRule>,
    metadata: PatternMetadata,
}

impl JsonPattern {
    /// Deserialize and validate a pattern from raw JSON bytes.
    ///
    /// `validators` is used to check whether a referenced validator name
    /// is registered: unrecognised names produce a [`JsonPatternWarning`]
    /// but do not prevent loading.
    ///
    /// On success returns the pattern together with a (possibly empty)
    /// list of [`JsonPatternWarning`]s.
    ///
    /// # Errors
    ///
    /// Returns [`JsonPatternError`] if the bytes cannot be parsed as JSON
    /// or do not match the expected schema (e.g. missing both `pattern`
    /// and `dictionary`).
    pub(crate) fn from_bytes(
        bytes: &[u8],
        validators: &ValidatorResolver,
    ) -> Result<(Self, Vec<JsonPatternWarning>), JsonPatternError> {
        /// Serde helper: exactly one of `pattern` or `dictionary` must be present.
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum RawSource {
            Regex { pattern: RegexPattern },
            Dictionary { dictionary: DictionaryPattern },
        }

        /// Intermediate serde target that mirrors the on-disk JSON shape.
        #[derive(Deserialize)]
        struct Raw {
            name: String,
            category: EntityCategory,
            #[serde(rename = "entity_type")]
            entity_kind: EntityKind,
            #[serde(flatten)]
            source: RawSource,
            #[serde(default)]
            context: Option<ContextRule>,
            #[serde(default)]
            metadata: PatternMetadata,
        }

        let raw: Raw = serde_json::from_slice(bytes)?;

        let match_source = match raw.source {
            RawSource::Regex { pattern } => MatchSource::Regex(pattern),
            RawSource::Dictionary { dictionary } => MatchSource::Dictionary(dictionary),
        };

        let mut warnings = Vec::new();

        if let MatchSource::Regex(RegexPattern {
            validator: Some(ref v),
            ..
        }) = match_source
            && validators.resolve(v).is_none()
        {
            warnings.push(JsonPatternWarning::UnknownValidator {
                pattern: raw.name.clone(),
                validator: v.clone(),
            });
        }

        let p = Self {
            name: raw.name,
            category: raw.category,
            entity_kind: raw.entity_kind,
            match_source,
            context: raw.context,
            metadata: raw.metadata,
        };

        Ok((p, warnings))
    }
}

impl Pattern for JsonPattern {
    fn name(&self) -> &str {
        &self.name
    }

    fn category(&self) -> EntityCategory {
        self.category
    }

    fn entity_kind(&self) -> EntityKind {
        self.entity_kind
    }

    fn match_source(&self) -> &MatchSource {
        &self.match_source
    }

    fn context(&self) -> Option<&ContextRule> {
        self.context.as_ref()
    }

    fn metadata(&self) -> &PatternMetadata {
        &self.metadata
    }
}
