use serde::{Deserialize, Serialize};
use crate::types::{EntityCategory, RedactionMethod};

/// Per-entity-type override for the redaction method.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct EntityRedactionRule {
    pub entity_type: String,
    pub method: RedactionMethod,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub replacement: Option<String>,
}

/// Request-scoped description of what to redact.
///
/// Acts as the per-request equivalent of a stored [`Policy`](super::policy::Policy),
/// specifying categories, entity types, confidence thresholds, and
/// redaction methods for a single redaction invocation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct RedactionContext {
    /// Entity categories to scan for. Empty = all.
    #[serde(default)]
    pub categories: Vec<EntityCategory>,
    /// Specific entity type names (e.g. "ssn", "face", "address"). Empty = all within categories.
    #[serde(default)]
    pub entity_types: Vec<String>,
    /// Per-entity-type overrides for redaction method.
    #[serde(default)]
    pub rules: Vec<EntityRedactionRule>,
    /// Default method when no per-type rule matches.
    #[serde(default = "default_method")]
    pub default_method: RedactionMethod,
    /// Minimum confidence (0.0-1.0). Below = ignored.
    #[serde(default = "default_min_confidence")]
    pub min_confidence: f64,
    /// Enable image-based detection (faces, license plates).
    #[serde(default)]
    pub detect_images: bool,
    /// Free-form labels (e.g. "gdpr-request").
    #[serde(default)]
    pub labels: Vec<String>,
}

fn default_method() -> RedactionMethod {
    RedactionMethod::Mask
}

fn default_min_confidence() -> f64 {
    0.5
}

impl Default for RedactionContext {
    fn default() -> Self {
        Self {
            categories: Vec::new(),
            entity_types: Vec::new(),
            rules: Vec::new(),
            default_method: RedactionMethod::Mask,
            min_confidence: 0.5,
            detect_images: false,
            labels: Vec::new(),
        }
    }
}

impl RedactionContext {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_categories(mut self, categories: Vec<EntityCategory>) -> Self {
        self.categories = categories;
        self
    }

    pub fn with_entity_types(mut self, entity_types: Vec<String>) -> Self {
        self.entity_types = entity_types;
        self
    }

    pub fn with_rule(mut self, rule: EntityRedactionRule) -> Self {
        self.rules.push(rule);
        self
    }

    pub fn with_default_method(mut self, method: RedactionMethod) -> Self {
        self.default_method = method;
        self
    }

    pub fn with_min_confidence(mut self, confidence: f64) -> Self {
        self.min_confidence = confidence;
        self
    }

    pub fn with_detect_images(mut self, detect: bool) -> Self {
        self.detect_images = detect;
        self
    }

    /// Return the redaction method for a given entity type.
    ///
    /// Checks per-type rules first, falls back to `default_method`.
    pub fn method_for(&self, entity_type: &str) -> RedactionMethod {
        self.rules
            .iter()
            .find(|r| r.entity_type == entity_type)
            .map(|r| r.method)
            .unwrap_or(self.default_method)
    }

    /// Whether a detected entity should be processed given the context filters.
    pub fn should_process(
        &self,
        category: EntityCategory,
        entity_type: &str,
        confidence: f64,
    ) -> bool {
        if confidence < self.min_confidence {
            return false;
        }
        if !self.categories.is_empty() && !self.categories.contains(&category) {
            return false;
        }
        if !self.entity_types.is_empty()
            && !self.entity_types.iter().any(|t| t == entity_type)
        {
            return false;
        }
        true
    }
}
