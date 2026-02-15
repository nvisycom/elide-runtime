//! Content representation combining data with metadata
//!
//! This module provides the [`Content`] struct that combines [`ContentData`]
//! with optional [`ContentMetadata`] for complete content representation.

use derive_more::{AsRef, Deref};
use serde::{Deserialize, Serialize};

use super::ContentData;
use crate::error::Result;
use crate::fs::ContentMetadata;
use crate::path::ContentSource;

/// Complete content representation with data and metadata
///
/// This struct combines [`ContentData`] (the actual content bytes) with
/// optional [`ContentMetadata`] (path, extension info, etc.) to provide
/// a complete content representation.
///
/// # Examples
///
/// ```rust
/// use nvisy_core::io::{Content, ContentData};
/// use nvisy_core::fs::ContentMetadata;
/// use nvisy_core::path::ContentSource;
///
/// // Create content from data
/// let data = ContentData::from("Hello, world!");
/// let content = Content::new(data);
///
/// assert_eq!(content.size(), 13);
/// assert!(content.is_likely_text());
///
/// // Create content with metadata
/// let source = ContentSource::new();
/// let data = ContentData::from_text(source, "Sample text");
/// let metadata = ContentMetadata::with_path(source, "document.txt");
/// let content = Content::with_metadata(data, metadata);
///
/// assert_eq!(content.metadata().and_then(|m| m.filename()), Some("document.txt"));
/// ```
#[derive(Debug, Clone, PartialEq)]
#[derive(AsRef, Deref, Serialize, Deserialize)]
pub struct Content {
    /// The actual content data
    #[deref]
    #[as_ref]
    data: ContentData,
    /// Optional metadata about the content
    metadata: Option<ContentMetadata>,
}

impl From<ContentData> for Content {
    fn from(data: ContentData) -> Self {
        Self::new(data)
    }
}

impl Content {
    /// Create new content from data without metadata
    pub fn new(data: ContentData) -> Self {
        Self {
            data,
            metadata: None,
        }
    }

    /// Create new content with metadata
    pub fn with_metadata(data: ContentData, metadata: ContentMetadata) -> Self {
        Self {
            data,
            metadata: Some(metadata),
        }
    }

    /// Get the content data
    pub fn data(&self) -> &ContentData {
        &self.data
    }

    /// Get the content metadata if available
    pub fn metadata(&self) -> Option<&ContentMetadata> {
        self.metadata.as_ref()
    }

    /// Get the content source
    pub fn content_source(&self) -> ContentSource {
        self.data.content_source
    }

    /// Get the content as bytes
    pub fn as_bytes(&self) -> &[u8] {
        self.data.as_bytes()
    }

    /// Returns `true` if the content appears to be text.
    pub fn is_likely_text(&self) -> bool {
        self.data.is_likely_text()
    }

    /// Try to get the content as a string slice.
    ///
    /// # Errors
    ///
    /// Returns an error if the content is not valid UTF-8.
    pub fn as_str(&self) -> Result<&str> {
        self.data.as_str()
    }

    /// Get the file extension from metadata if available
    pub fn file_extension(&self) -> Option<&str> {
        self.metadata.as_ref().and_then(|m| m.file_extension())
    }

    /// Get the filename from metadata if available
    pub fn filename(&self) -> Option<&str> {
        self.metadata.as_ref().and_then(|m| m.filename())
    }

    /// Set the metadata
    pub fn set_metadata(&mut self, metadata: ContentMetadata) {
        self.metadata = Some(metadata);
    }

    /// Remove the metadata
    pub fn clear_metadata(&mut self) {
        self.metadata = None;
    }

    /// Consume and return the inner [`ContentData`].
    pub fn into_data(self) -> ContentData {
        self.data
    }

    /// Consume and return both data and metadata
    pub fn into_parts(self) -> (ContentData, Option<ContentMetadata>) {
        (self.data, self.metadata)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_content_creation() {
        let data = ContentData::from("Hello, world!");
        let content = Content::new(data.clone());

        assert_eq!(content.size(), 13);
        assert!(content.is_likely_text());
        assert!(content.metadata().is_none());
    }

    #[test]
    fn test_content_with_metadata() {
        let source = ContentSource::new();
        let data = ContentData::from_text(source, "Test content");
        let metadata = ContentMetadata::with_path(source, "test.txt");
        let content = Content::with_metadata(data, metadata);

        assert!(content.metadata().is_some());
        assert_eq!(content.file_extension(), Some("txt"));
        assert_eq!(content.filename(), Some("test.txt"));
    }

    #[test]
    fn test_content_deref() {
        let data = ContentData::from("Hello");
        let content = Content::new(data);

        // Test that Deref works - we can call ContentData methods directly
        assert_eq!(content.size(), 5);
        assert_eq!(content.as_str().unwrap(), "Hello");
    }

    #[test]
    fn test_content_from() {
        let data = ContentData::from("Test");
        let content: Content = data.into();

        assert_eq!(content.size(), 4);
    }

    #[test]
    fn test_metadata_operations() {
        let data = ContentData::from("Test");
        let mut content = Content::new(data);

        assert!(content.metadata().is_none());

        let source = content.content_source();
        let metadata = ContentMetadata::with_path(source, "file.pdf");
        content.set_metadata(metadata);

        assert!(content.metadata().is_some());
        assert_eq!(content.file_extension(), Some("pdf"));

        content.clear_metadata();
        assert!(content.metadata().is_none());
    }

    #[test]
    fn test_into_parts() {
        let source = ContentSource::new();
        let data = ContentData::from_text(source, "Test");
        let metadata = ContentMetadata::with_path(source, "test.txt");
        let content = Content::with_metadata(data.clone(), metadata.clone());

        let (recovered_data, recovered_metadata) = content.into_parts();
        assert_eq!(recovered_data, data);
        assert_eq!(recovered_metadata, Some(metadata));
    }

    #[test]
    fn test_serialization() {
        let data = ContentData::from("Test content");
        let content = Content::new(data);

        let json = serde_json::to_string(&content).unwrap();
        let deserialized: Content = serde_json::from_str(&json).unwrap();

        assert_eq!(content, deserialized);
    }

    #[test]
    fn test_content_source() {
        let source = ContentSource::new();
        let data = ContentData::from_text(source, "Test");
        let content = Content::new(data);

        assert_eq!(content.content_source(), source);
    }
}
