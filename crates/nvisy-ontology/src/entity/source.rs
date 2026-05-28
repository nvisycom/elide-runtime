//! Content source identity and lineage.

use derive_more::Display;
use jiff::Zoned;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Unique identifier for content sources in the system.
///
/// Uses `UUIDv7` for time-ordered, globally unique identification of data
/// sources. Tracks parent lineage for provenance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[derive(Serialize, Deserialize, JsonSchema, Display)]
#[display("{id}")]
pub struct ContentSource {
    id: Uuid,
    #[serde(skip_serializing_if = "Option::is_none")]
    parent_id: Option<Uuid>,
}

impl ContentSource {
    /// Create a new content source with a fresh UUIDv7.
    #[must_use]
    pub fn new() -> Self {
        let now = Zoned::now();
        let timestamp = uuid::Timestamp::from_unix(
            uuid::NoContext,
            now.timestamp().as_second().unsigned_abs(),
            now.timestamp().subsec_nanosecond().unsigned_abs(),
        );
        Self {
            id: Uuid::new_v7(timestamp),
            parent_id: None,
        }
    }

    /// Create a content source from an existing UUID.
    #[must_use]
    pub fn from_uuid(id: Uuid) -> Self {
        Self {
            id,
            parent_id: None,
        }
    }

    /// Get the underlying UUID.
    #[must_use]
    pub fn as_uuid(&self) -> Uuid {
        self.id
    }

    /// Get the parent source identifier, if any.
    #[must_use]
    pub fn parent_id(&self) -> Option<Uuid> {
        self.parent_id
    }

    /// Create a copy with the given parent (builder pattern).
    #[must_use]
    pub fn with_parent(mut self, parent: &ContentSource) -> Self {
        self.parent_id = Some(parent.id);
        self
    }
}

impl Default for ContentSource {
    fn default() -> Self {
        Self::new()
    }
}

impl From<Uuid> for ContentSource {
    fn from(id: Uuid) -> Self {
        Self::from_uuid(id)
    }
}

impl From<ContentSource> for Uuid {
    fn from(source: ContentSource) -> Self {
        source.id
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_is_uuidv7() {
        let source = ContentSource::new();
        assert_eq!(source.as_uuid().get_version_num(), 7);
        assert!(!source.as_uuid().is_nil());
    }

    #[test]
    fn with_parent_sets_parent_id() {
        let parent = ContentSource::new();
        let child = ContentSource::new().with_parent(&parent);
        assert_eq!(child.parent_id(), Some(parent.as_uuid()));
        assert_ne!(child.as_uuid(), parent.as_uuid());
    }
}
