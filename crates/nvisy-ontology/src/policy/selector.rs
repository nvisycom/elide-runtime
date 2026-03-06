//! Entity selection criteria for policy rules.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::entity::{EntityCategory, EntityKind};

/// Criteria for selecting which entities a policy rule applies to.
///
/// All fields use "empty means all" semantics: an empty `categories` list
/// matches every category, an empty `entity_types` list matches every type,
/// and so on. When multiple fields are set, they are combined with AND logic.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct EntitySelector {
    /// Entity categories this selector matches. Empty means all categories.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub entity_categories: Vec<EntityCategory>,
    /// Specific entity kinds this selector matches. Empty means all kinds.
    #[serde(
        rename = "entity_types",
        default,
        skip_serializing_if = "Vec::is_empty"
    )]
    pub entity_kinds: Vec<EntityKind>,
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
            entity_categories: Vec::new(),
            entity_kinds: Vec::new(),
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
    pub fn matches(
        &self,
        category: &EntityCategory,
        entity_kind: EntityKind,
        confidence: f64,
    ) -> bool {
        if confidence < self.confidence_threshold {
            return false;
        }
        if !self.entity_categories.is_empty() && !self.entity_categories.contains(category) {
            return false;
        }
        if !self.entity_kinds.is_empty() && !self.entity_kinds.contains(&entity_kind) {
            return false;
        }
        true
    }
}
