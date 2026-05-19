//! Redaction map: entity-to-value mapping for tracking original and
//! replacement content across all modalities.
//!
//! The [`RedactionMap`] is created during the redaction phase and
//! contains the sensitive original values that are stripped from the
//! [`Audit`] response. It is stored separately and access-controlled.
//!
//! [`Audit`]: super::Audit

use derive_more::{Deref, DerefMut};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::entity::Location;

/// A single entry in the redaction map, tracking the original and
/// replacement values for one entity.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RedactionMapping {
    /// The entity this mapping belongs to.
    pub entity_id: Uuid,
    /// Where in the document the entity was found.
    pub location: Location,
    /// The original sensitive value at this location.
    pub original: String,
    /// The replacement value after redaction, if applied.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub replacement: Option<String>,
}

/// Maps entity IDs to their original and replacement values.
///
/// Created during the redaction phase (phase 4) by the redaction
/// evaluator and applicator. Contains sensitive data — must not be
/// included in the public [`Audit`] response. Stored separately
/// under access control.
///
/// [`Audit`]: super::Audit
#[derive(Debug, Clone, Default, Deref, DerefMut)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RedactionMap {
    /// Per-entity redaction mappings.
    #[deref]
    #[deref_mut]
    pub entries: Vec<RedactionMapping>,
}

impl RedactionMap {
    /// Create an empty redaction map.
    pub fn new() -> Self {
        Self::default()
    }

    /// Look up the original value for a given entity.
    pub fn original(&self, entity_id: Uuid) -> Option<&str> {
        self.entries
            .iter()
            .find(|e| e.entity_id == entity_id)
            .map(|e| e.original.as_str())
    }

    /// Look up the replacement value for a given entity.
    pub fn replacement(&self, entity_id: Uuid) -> Option<&str> {
        self.entries
            .iter()
            .find(|e| e.entity_id == entity_id)
            .and_then(|e| e.replacement.as_deref())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::{Location, TextLocation};

    fn mapping(id: Uuid, original: &str, replacement: Option<&str>) -> RedactionMapping {
        RedactionMapping {
            entity_id: id,
            location: Location::from(TextLocation {
                start_offset: 0,
                end_offset: original.len(),
                ..Default::default()
            }),
            original: original.to_string(),
            replacement: replacement.map(String::from),
        }
    }

    #[test]
    fn push_and_lookup() {
        let id = Uuid::now_v7();
        let mut map = RedactionMap::new();
        map.push(mapping(id, "John", Some("[NAME]")));
        assert_eq!(map.len(), 1);
        assert_eq!(map.original(id), Some("John"));
        assert_eq!(map.replacement(id), Some("[NAME]"));
    }
}
