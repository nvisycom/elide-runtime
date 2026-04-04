//! Redaction map: entity-to-value mapping for tracking original and
//! replacement content across all modalities.
//!
//! The [`RedactionMap`] is created during the redaction phase and
//! contains the sensitive original values that are stripped from the
//! [`Audit`] response. It is stored separately and access-controlled.
//!
//! [`Audit`]: super::Audit

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
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RedactionMap {
    /// Per-entity redaction mappings.
    pub entries: Vec<RedactionMapping>,
}

impl RedactionMap {
    /// Create an empty redaction map.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a mapping entry.
    pub fn push(&mut self, mapping: RedactionMapping) {
        self.entries.push(mapping);
    }

    /// Number of entries in the map.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the map is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
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
