//! Entity selection criteria for policy rules.
//!
//! An [`EntitySelector`] describes which entities a policy rule or redaction
//! applies to, based on category, type, and confidence constraints.

use serde::{Deserialize, Serialize};

use super::EntityCategory;

/// Criteria for selecting which entities a policy rule applies to.
///
/// All fields use "empty means all" semantics: an empty `categories` list
/// matches every category, an empty `entity_types` list matches every type,
/// and so on. When multiple fields are set, they are combined with AND logic.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "jsonschema", derive(schemars::JsonSchema))]
pub struct EntitySelector {
    /// Entity categories this selector matches. Empty means all categories.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub categories: Vec<EntityCategory>,
    /// Specific entity type names this selector matches. Empty means all types.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub entity_types: Vec<String>,
    /// Minimum detection confidence required. Entities below this threshold
    /// are not matched.
    #[serde(default = "default_confidence_threshold")]
    pub confidence_threshold: f64,
}

fn default_confidence_threshold() -> f64 {
    0.0
}

impl Default for EntitySelector {
    fn default() -> Self {
        Self {
            categories: Vec::new(),
            entity_types: Vec::new(),
            confidence_threshold: default_confidence_threshold(),
        }
    }
}

impl EntitySelector {
    /// Create a selector that matches all entities.
    pub fn all() -> Self {
        Self::default()
    }

    /// Returns `true` if the given entity properties match this selector.
    pub fn matches(&self, category: &EntityCategory, entity_type: &str, confidence: f64) -> bool {
        if confidence < self.confidence_threshold {
            return false;
        }
        if !self.categories.is_empty() && !self.categories.contains(category) {
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
