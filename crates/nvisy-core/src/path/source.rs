//! Content source identification module
//!
//! This module provides the [`ContentSource`] struct for uniquely identifying
//! data sources throughout the nvisy system using `UUIDv7`.

use std::fmt;

use jiff::Zoned;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::{Error, ErrorKind, Result};

/// Unique identifier for content sources in the system
///
/// Uses `UUIDv7` for time-ordered, globally unique identification of data sources.
///
/// This allows for efficient tracking and correlation of content throughout
/// the processing pipeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[derive(Serialize, Deserialize, JsonSchema)]
pub struct ContentSource {
    /// `UUIDv7` identifier
    id: Uuid,
    /// Optional parent source for lineage tracking
    #[serde(skip_serializing_if = "Option::is_none")]
    parent_id: Option<Uuid>,
}

impl ContentSource {
    /// Create a new content source with a fresh `UUIDv7`
    ///
    /// # Example
    ///
    /// ```
    /// use nvisy_core::path::ContentSource;
    ///
    /// let source = ContentSource::new();
    /// assert!(!source.as_uuid().is_nil());
    /// ```
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

    /// Create a content source from an existing UUID
    ///
    /// # Example
    ///
    /// ```
    /// use nvisy_core::path::ContentSource;
    /// use uuid::Uuid;
    ///
    /// let source = ContentSource::new();
    /// let uuid = source.as_uuid();
    /// let source2 = ContentSource::from_uuid(uuid);
    /// assert_eq!(source2.as_uuid(), uuid);
    /// ```
    #[must_use]
    pub fn from_uuid(id: Uuid) -> Self {
        Self {
            id,
            parent_id: None,
        }
    }

    /// Get the underlying UUID
    ///
    /// # Example
    ///
    /// ```
    /// use nvisy_core::path::ContentSource;
    ///
    /// let source = ContentSource::new();
    /// let uuid = source.as_uuid();
    /// assert_eq!(uuid.get_version_num(), 7);
    /// ```
    #[must_use]
    pub fn as_uuid(&self) -> Uuid {
        self.id
    }

    /// Parse a content source from a UUID string.
    ///
    /// # Errors
    ///
    /// Returns an error if the string is not a valid UUID format.
    ///
    /// # Example
    ///
    /// ```
    /// use nvisy_core::path::ContentSource;
    ///
    /// let source = ContentSource::new();
    /// let id_str = source.to_string();
    /// let parsed = ContentSource::parse(&id_str).unwrap();
    /// assert_eq!(source, parsed);
    /// ```
    pub fn parse(s: &str) -> Result<Self> {
        let id = Uuid::parse_str(s).map_err(|err| {
            Error::new(
                ErrorKind::Validation,
                format!("Invalid content source UUID: {err}"),
            )
        })?;
        Ok(Self {
            id,
            parent_id: None,
        })
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

    /// Create a copy of this source with the given parent (builder pattern).
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

    /// Get the timestamp component from the `UUIDv7`
    ///
    /// Returns the Unix timestamp in milliseconds when this UUID was generated,
    /// or None if this is not a `UUIDv7`.
    ///
    /// # Example
    ///
    /// ```
    /// use nvisy_core::path::ContentSource;
    /// use std::time::{SystemTime, UNIX_EPOCH};
    ///
    /// let source = ContentSource::new();
    /// let timestamp = source.timestamp().expect("UUIDv7 should have timestamp");
    /// let now = SystemTime::now()
    ///     .duration_since(UNIX_EPOCH)
    ///     .unwrap()
    ///     .as_millis() as u64;
    ///
    /// // Should be very close to current time (within a few seconds)
    /// assert!((timestamp as i64 - now as i64).abs() < 5000);
    /// ```
    #[must_use]
    pub fn timestamp(&self) -> Option<u64> {
        self.id.get_timestamp().map(|timestamp| {
            let (seconds, nanos) = timestamp.to_unix();
            seconds * 1000 + u64::from(nanos) / 1_000_000
        })
    }

    /// Check if this content source was created before another
    ///
    /// Returns false if either UUID is not a `UUIDv7` and thus has no timestamp.
    ///
    /// # Example
    ///
    /// ```
    /// use nvisy_core::path::ContentSource;
    /// use std::thread;
    /// use std::time::Duration;
    ///
    /// let source1 = ContentSource::new();
    /// thread::sleep(Duration::from_millis(1));
    /// let source2 = ContentSource::new();
    ///
    /// assert!(source1.created_before(&source2));
    /// assert!(!source2.created_before(&source1));
    /// ```
    #[must_use]
    pub fn created_before(&self, other: &ContentSource) -> bool {
        match (self.timestamp(), other.timestamp()) {
            (Some(self_ts), Some(other_ts)) => self_ts < other_ts,
            _ => false,
        }
    }

    /// Check if this content source was created after another
    ///
    /// Returns false if either UUID is not a `UUIDv7` and thus has no timestamp.
    #[must_use]
    pub fn created_after(&self, other: &ContentSource) -> bool {
        match (self.timestamp(), other.timestamp()) {
            (Some(self_ts), Some(other_ts)) => self_ts > other_ts,
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
    use crate::error::ErrorKind;

    #[test]
    fn test_new_content_source() {
        let source = ContentSource::new();
        assert_eq!(source.as_uuid().get_version_num(), 7);
        assert!(!source.as_uuid().is_nil());
    }

    #[test]
    fn test_uniqueness() {
        let mut sources = HashSet::new();

        // Generate 1000 sources and ensure they're all unique
        for _ in 0..1000 {
            let source = ContentSource::new();
            assert!(sources.insert(source), "Duplicate content source found");
        }
    }

    #[test]
    fn test_string_conversion() {
        let source = ContentSource::new();
        let string_repr = source.to_string();
        let parsed = ContentSource::parse(&string_repr).unwrap();
        assert_eq!(source, parsed);
    }

    #[test]
    fn test_invalid_string_parsing() {
        let err = ContentSource::parse("invalid-uuid").unwrap_err();
        assert_eq!(err.kind, ErrorKind::Validation);
    }

    #[test]
    fn test_ordering() {
        let source1 = ContentSource::new();
        thread::sleep(Duration::from_millis(2));
        let source2 = ContentSource::new();

        assert!(source1.created_before(&source2));
        assert!(source2.created_after(&source1));
        assert!(source1 < source2); // Test PartialOrd
    }

    #[test]
    fn test_serde_serialization() {
        let source = ContentSource::new();
        let serialized = serde_json::to_string(&source).unwrap();
        let deserialized: ContentSource = serde_json::from_str(&serialized).unwrap();
        assert_eq!(source, deserialized);
    }
}
