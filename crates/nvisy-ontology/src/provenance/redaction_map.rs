//! Redaction map: entity-to-location index for the redaction phase.
//!
//! The [`RedactionMap`] records which entities the pipeline touched
//! and where they were located. Original and replacement *values*
//! live on the corresponding [`AuditEntry::value`] (see
//! [`RedactionValue`]) — the map is a thin index, not a value store.
//!
//! A future extension may pair this index with a separate blob store
//! to support reversibility for image/audio modalities — see
//! [issue #151](https://github.com/nvisycom/runtime/issues/151).
//!
//! [`AuditEntry::value`]: super::AuditEntry::value
//! [`RedactionValue`]: super::RedactionValue

use derive_more::{Deref, DerefMut};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::entity::Location;

/// One entry in the redaction map: the entity touched and where it
/// was located in the document.
///
/// Values (original / replacement) are not stored here; consult the
/// corresponding [`AuditEntry`] by `entity_id`.
///
/// [`AuditEntry`]: super::AuditEntry
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RedactionMapping {
    /// The entity this mapping belongs to.
    pub entity_id: Uuid,
    /// Where in the document the entity was found.
    pub location: Location,
}

/// Per-entity redaction lineage index.
///
/// Created during the redaction phase (phase 4) by the redaction
/// evaluator. Records which entities were considered for redaction
/// and where they lived in the document. Sensitive values are not
/// duplicated here — they live on the matching [`AuditEntry`].
///
/// [`AuditEntry`]: super::AuditEntry
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::{Location, TextLocation};

    fn mapping(id: Uuid) -> RedactionMapping {
        RedactionMapping {
            entity_id: id,
            location: Location::from(TextLocation {
                start_offset: 0,
                end_offset: 4,
                ..Default::default()
            }),
        }
    }

    #[test]
    fn push_and_count() {
        let id = Uuid::now_v7();
        let mut map = RedactionMap::new();
        map.push(mapping(id));
        assert_eq!(map.len(), 1);
        assert_eq!(map.entries[0].entity_id, id);
    }
}
