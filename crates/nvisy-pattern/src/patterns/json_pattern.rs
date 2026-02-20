//! JSON-backed pattern implementation.

use nvisy_core::data::{EntityCategory, EntityKind};

use super::pattern::Pattern;
use super::validators;

/// A regex-based detection pattern loaded from a JSON definition file.
#[derive(Debug, Clone)]
pub struct JsonPattern {
    name: String,
    category: EntityCategory,
    entity_kind: EntityKind,
    pattern_str: String,
    confidence: f64,
    validator: Option<String>,
}

impl JsonPattern {
    /// Create a new JSON-backed pattern (crate-internal).
    pub(crate) fn new(
        name: String,
        category: EntityCategory,
        entity_kind: EntityKind,
        pattern_str: String,
        confidence: f64,
        validator: Option<String>,
    ) -> Self {
        Self {
            name,
            category,
            entity_kind,
            pattern_str,
            confidence,
            validator,
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
