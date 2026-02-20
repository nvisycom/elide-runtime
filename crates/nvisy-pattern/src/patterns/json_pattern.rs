//! JSON-backed pattern implementation.

use serde::Deserialize;

use nvisy_core::data::{EntityCategory, EntityKind};

use super::pattern::Pattern;
use super::validators;

/// A regex-based detection pattern loaded from a JSON definition file.
#[derive(Debug, Clone, Deserialize)]
pub struct JsonPattern {
    name: String,
    category: EntityCategory,
    #[serde(rename = "entity_type")]
    entity_kind: EntityKind,
    #[serde(rename = "pattern")]
    pattern_str: String,
    confidence: f64,
    #[serde(default)]
    validator: Option<String>,
}

impl JsonPattern {
    /// Warn if the category fell through to `Custom` or the validator is unknown.
    pub(crate) fn warn_on_load(&self) {
        if let EntityCategory::Custom(ref slug) = self.category {
            tracing::warn!(
                pattern = %self.name,
                category = %slug,
                "unrecognised category falls through to Custom",
            );
        }
        if let Some(ref v) = self.validator {
            if validators::resolve(v).is_none() {
                tracing::warn!(
                    pattern = %self.name,
                    validator = %v,
                    "unknown validator name, pattern will have no post-match validation",
                );
            }
        }
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

    fn pattern_str(&self) -> &str {
        &self.pattern_str
    }

    fn confidence(&self) -> f64 {
        self.confidence
    }

    fn validate(&self, value: &str) -> bool {
        match self.validator.as_deref().and_then(validators::resolve) {
            Some(f) => f(value),
            None => true,
        }
    }

    fn has_validator(&self) -> bool {
        self.validator.is_some()
    }
}
