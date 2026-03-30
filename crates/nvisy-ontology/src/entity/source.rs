//! Content source identity and lineage.

use std::fmt;

use jiff::Zoned;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Unique identifier for content sources in the system.
///
/// Uses `UUIDv7` for time-ordered, globally unique identification of data
/// sources. Tracks parent lineage for provenance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[derive(Serialize, Deserialize, JsonSchema)]
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

    /// Set the parent source identifier.
    pub fn set_parent_id(&mut self, parent_id: Option<Uuid>) {
        self.parent_id = parent_id;
    }

    /// Create a copy with the given parent (builder pattern).
    #[must_use]
    pub fn with_parent(mut self, parent: &ContentSource) -> Self {
        self.parent_id = Some(parent.id);
        self
    }

    /// Create a new content source derived from this one (new ID, self as parent).
    #[must_use]
    pub fn derive(&self) -> Self {
        Self::new().with_parent(self)
    }

    /// Get the timestamp component from the UUIDv7.
    ///
    /// Returns the Unix timestamp in milliseconds, or `None` if not a UUIDv7.
    #[must_use]
    pub fn timestamp(&self) -> Option<u64> {
        self.id.get_timestamp().map(|timestamp| {
            let (seconds, nanos) = timestamp.to_unix();
            seconds * 1000 + u64::from(nanos) / 1_000_000
        })
    }

    /// Returns `true` if this source was created before `other`.
    #[must_use]
    pub fn created_before(&self, other: &ContentSource) -> bool {
        match (self.timestamp(), other.timestamp()) {
            (Some(a), Some(b)) => a < b,
            _ => false,
        }
    }

    /// Returns `true` if this source was created after `other`.
    #[must_use]
    pub fn created_after(&self, other: &ContentSource) -> bool {
        match (self.timestamp(), other.timestamp()) {
            (Some(a), Some(b)) => a > b,
            _ => false,
        }
    }
}

impl Default for ContentSource {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for ContentSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.id)
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
    use std::collections::HashSet;
    use std::thread;
    use std::time::Duration;

    use super::*;

    #[test]
    fn new_is_uuidv7() {
        let source = ContentSource::new();
        assert_eq!(source.as_uuid().get_version_num(), 7);
        assert!(!source.as_uuid().is_nil());
    }

    #[test]
    fn uniqueness() {
        let mut sources = HashSet::new();
        for _ in 0..1000 {
            assert!(sources.insert(ContentSource::new()));
        }
    }

    #[test]
    fn derive_sets_parent() {
        let parent = ContentSource::new();
        let child = parent.derive();
        assert_eq!(child.parent_id(), Some(parent.as_uuid()));
        assert_ne!(child.as_uuid(), parent.as_uuid());
    }

    #[test]
    fn ordering() {
        let a = ContentSource::new();
        thread::sleep(Duration::from_millis(2));
        let b = ContentSource::new();
        assert!(a.created_before(&b));
        assert!(a < b);
    }

    #[test]
    fn serde_roundtrip() {
        let source = ContentSource::new();
        let json = serde_json::to_string(&source).unwrap();
        let d: ContentSource = serde_json::from_str(&json).unwrap();
        assert_eq!(source, d);
    }
}
