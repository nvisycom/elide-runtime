//! Entity selection criteria for policy rules.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::entity::{Entity, EntityCategory, EntityKind, EntitySensitivity};

/// Criteria for selecting which entities a policy rule applies to.
///
/// All fields use "empty means all" semantics: an empty `categories` list
/// matches every category, an empty `entity_types` list matches every type,
/// and so on. When multiple fields are set, they are combined with AND logic.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct EntitySelector {
    /// Entity categories this selector matches. Empty means all categories.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub entity_categories: Vec<EntityCategory>,
    /// Specific entity kinds this selector matches. Empty means all kinds.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub entity_kinds: Vec<EntityKind>,
    /// Sensitivity levels this selector matches. Empty means all levels.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sensitivities: Vec<EntitySensitivity>,
    /// Minimum detection confidence required. Entities below this threshold
    /// are not matched. `None` means no threshold (matches any confidence).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confidence_threshold: Option<f64>,
}

impl EntitySelector {
    /// Create a selector that matches all entities.
    pub fn all() -> Self {
        Self::default()
    }

    /// Returns `true` if the given entity matches this selector.
    pub fn matches(&self, entity: &Entity) -> bool {
        if let Some(threshold) = self.confidence_threshold
            && entity.confidence < threshold
        {
            return false;
        }
        if !self.entity_categories.is_empty() && !self.entity_categories.contains(&entity.category)
        {
            return false;
        }
        if !self.entity_kinds.is_empty() && !self.entity_kinds.contains(&entity.entity_kind) {
            return false;
        }
        if !self.sensitivities.is_empty() {
            match entity.sensitivity {
                Some(s) if self.sensitivities.contains(&s) => {}
                _ => return false,
            }
        }

        true
    }
}
