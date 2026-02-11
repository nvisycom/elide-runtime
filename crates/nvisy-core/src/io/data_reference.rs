//! Data reference definitions
//!
//! This module provides the `DataReference` struct for referencing and
//! tracking content within the Nvisy system.

use serde::{Deserialize, Serialize};

use crate::io::Content;
use crate::path::ContentSource;

/// Reference to data with source tracking and content information
///
/// A `DataReference` provides a lightweight way to reference data content
/// while maintaining information about its source location and optional
/// mapping within that source.
///
/// # Examples
///
/// ```rust
/// use nvisy_core::io::{DataReference, Content, ContentData};
///
/// let content = Content::new(ContentData::from("Hello, world!"));
/// let data_ref = DataReference::new(content)
///     .with_mapping_id("line-42");
///
/// assert!(data_ref.mapping_id().is_some());
/// assert_eq!(data_ref.mapping_id().unwrap(), "line-42");
/// ```
#[derive(Debug, Clone)]
#[derive(Serialize, Deserialize)]
pub struct DataReference {
    /// Unique identifier for the source containing this data
    /// Using `UUIDv7` for time-ordered, globally unique identification
    source: ContentSource,

    /// Optional identifier that defines the position/location of the data within the source
    /// Examples: line numbers, byte offsets, element IDs, `XPath` expressions
    mapping_id: Option<String>,

    /// The actual content data
    content: Content,
}

impl DataReference {
    /// Create a new data reference with auto-generated source ID (`UUIDv7`)
    pub fn new(content: Content) -> Self {
        Self {
            source: ContentSource::new(),
            mapping_id: None,
            content,
        }
    }

    /// Create a new data reference with specific source
    pub fn with_source(source: ContentSource, content: Content) -> Self {
        Self {
            source,
            mapping_id: None,
            content,
        }
    }

    /// Set the mapping ID for this data reference
    #[must_use]
    pub fn with_mapping_id<S: Into<String>>(mut self, mapping_id: S) -> Self {
        self.mapping_id = Some(mapping_id.into());
        self
    }

    /// Get the content source
    pub fn source(&self) -> ContentSource {
        self.source
    }

    /// Get the mapping ID, if any
    pub fn mapping_id(&self) -> Option<&str> {
        self.mapping_id.as_deref()
    }

    /// Get a reference to the content
    pub fn content(&self) -> &Content {
        &self.content
    }

    /// Check if the content is text-based
    pub fn is_likely_text(&self) -> bool {
        self.content.is_likely_text()
    }

    /// Get the size of the content in bytes
    pub fn size(&self) -> usize {
        self.content.size()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::io::ContentData;

    #[test]
    fn test_data_reference_creation() {
        let content = Content::new(ContentData::from("Hello, world!"));
        let data_ref = DataReference::new(content);

        assert!(data_ref.is_likely_text());
        assert!(data_ref.mapping_id().is_none());
        assert_eq!(data_ref.size(), 13);
        // Verify UUIDv7 is used
        assert_eq!(data_ref.source().as_uuid().get_version_num(), 7);
    }

    #[test]
    fn test_data_reference_with_mapping() {
        let content = Content::new(ContentData::from("Test content"));
        let data_ref = DataReference::new(content).with_mapping_id("line-42");

        assert_eq!(data_ref.mapping_id(), Some("line-42"));
    }

    #[test]
    fn test_data_reference_with_source() {
        let source = ContentSource::new();
        let content = Content::new(ContentData::from("Test content"));
        let data_ref = DataReference::with_source(source, content);

        assert_eq!(data_ref.source(), source);
    }

    #[test]
    fn test_serialization() {
        let content = Content::new(ContentData::from("Test content"));
        let data_ref = DataReference::new(content).with_mapping_id("test-mapping");

        let json = serde_json::to_string(&data_ref).unwrap();
        let deserialized: DataReference = serde_json::from_str(&json).unwrap();

        assert_eq!(data_ref.source(), deserialized.source());
        assert_eq!(data_ref.mapping_id(), deserialized.mapping_id());
    }
}
