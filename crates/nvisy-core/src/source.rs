//! Content source identity and lineage.

use derive_more::Display;
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
    ///
    /// The timestamp comes from [`Uuid::now_v7`], which wraps
    /// `SystemTime::now()` and treats the duration since the Unix
    /// epoch as the timestamp source — no zoned-time round-trip,
    /// no separate sign handling for pre-epoch clocks (the standard
    /// library returns those as `Err` rather than as a negative
    /// `i64` to be silently `unsigned_abs`'d).
    #[must_use]
    pub fn new() -> Self {
        Self {
            id: Uuid::now_v7(),
            parent_id: None,
        }
    }

    /// Create a content source from an existing UUID **without
    /// validating the version**.
    ///
    /// [`ContentSource::new`] only ever mints UUIDv7 (timestamp-
    /// ordered), and the rest of the ontology — sort order on
    /// `Audit::records`, lineage debugging, lexicographic comparison
    /// across runs — depends on that ordering. Use this constructor
    /// only when the caller has out-of-band confidence the UUID was
    /// produced by an earlier `ContentSource::new` (e.g. read back
    /// from the engine's registry, propagated across a process
    /// boundary). Passing a v4 / v1 / nil UUID compiles, runs, and
    /// silently breaks downstream invariants that only manifest at
    /// inspection time.
    #[must_use]
    pub fn from_uuid_unchecked(id: Uuid) -> Self {
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
        Self::from_uuid_unchecked(id)
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
    fn with_parent_sets_parent_id() {
        let parent = ContentSource::new();
        let child = ContentSource::new().with_parent(&parent);
        assert_eq!(child.parent_id(), Some(parent.as_uuid()));
        assert_ne!(child.as_uuid(), parent.as_uuid());
    }
}
