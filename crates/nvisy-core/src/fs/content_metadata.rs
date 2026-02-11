//! Content metadata for filesystem operations
//!
//! This module provides the [`ContentMetadata`] struct for handling metadata
//! about content files, including paths and source tracking.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::path::ContentSource;

/// Metadata associated with content files
///
/// This struct stores metadata about content including its source identifier
/// and file path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContentMetadata {
    /// Unique identifier for the content source
    pub content_source: ContentSource,
    /// Optional path to the source file
    pub source_path: Option<PathBuf>,
}

impl ContentMetadata {
    /// Create new content metadata with just a source
    ///
    /// # Example
    ///
    /// ```
    /// use nvisy_core::{fs::ContentMetadata, path::ContentSource};
    ///
    /// let source = ContentSource::new();
    /// let metadata = ContentMetadata::new(source);
    /// ```
    #[must_use]
    pub fn new(content_source: ContentSource) -> Self {
        Self {
            content_source,
            source_path: None,
        }
    }

    /// Create content metadata with a file path
    ///
    /// # Example
    ///
    /// ```
    /// use nvisy_core::{fs::ContentMetadata, path::ContentSource};
    /// use std::path::PathBuf;
    ///
    /// let source = ContentSource::new();
    /// let metadata = ContentMetadata::with_path(source, PathBuf::from("document.pdf"));
    /// assert_eq!(metadata.file_extension(), Some("pdf"));
    /// ```
    pub fn with_path(content_source: ContentSource, path: impl Into<PathBuf>) -> Self {
        Self {
            content_source,
            source_path: Some(path.into()),
        }
    }

    /// Get the file extension if available
    #[must_use]
    pub fn file_extension(&self) -> Option<&str> {
        self.source_path
            .as_ref()
            .and_then(|path| path.extension())
            .and_then(|ext| ext.to_str())
    }

    /// Get the filename if available
    #[must_use]
    pub fn filename(&self) -> Option<&str> {
        self.source_path
            .as_ref()
            .and_then(|path| path.file_name())
            .and_then(|name| name.to_str())
    }

    /// Get the parent directory if available
    #[must_use]
    pub fn parent_directory(&self) -> Option<&Path> {
        self.source_path.as_ref().and_then(|path| path.parent())
    }

    /// Get the full path if available
    #[must_use]
    pub fn path(&self) -> Option<&Path> {
        self.source_path.as_deref()
    }

    /// Set the source path
    pub fn set_path(&mut self, path: impl Into<PathBuf>) {
        self.source_path = Some(path.into());
    }

    /// Remove the source path
    pub fn clear_path(&mut self) {
        self.source_path = None;
    }

    /// Check if this metadata has a path
    #[must_use]
    pub fn has_path(&self) -> bool {
        self.source_path.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_content_metadata_creation() {
        let source = ContentSource::new();
        let metadata = ContentMetadata::new(source);

        assert_eq!(metadata.content_source, source);
        assert!(metadata.source_path.is_none());
        assert!(!metadata.has_path());
    }

    #[test]
    fn test_content_metadata_with_path() {
        let source = ContentSource::new();
        let path = PathBuf::from("/path/to/document.pdf");
        let metadata = ContentMetadata::with_path(source, path.clone());

        assert_eq!(metadata.content_source, source);
        assert_eq!(metadata.source_path, Some(path));
        assert!(metadata.has_path());
    }

    #[test]
    fn test_file_extension_detection() {
        let source = ContentSource::new();
        let metadata = ContentMetadata::with_path(source, PathBuf::from("document.pdf"));

        assert_eq!(metadata.file_extension(), Some("pdf"));
    }

    #[test]
    fn test_metadata_filename() {
        let source = ContentSource::new();
        let metadata = ContentMetadata::with_path(source, PathBuf::from("/path/to/file.txt"));

        assert_eq!(metadata.filename(), Some("file.txt"));
    }

    #[test]
    fn test_metadata_parent_directory() {
        let source = ContentSource::new();
        let metadata = ContentMetadata::with_path(source, PathBuf::from("/path/to/file.txt"));

        assert_eq!(metadata.parent_directory(), Some(Path::new("/path/to")));
    }

    #[test]
    fn test_path_operations() {
        let source = ContentSource::new();
        let mut metadata = ContentMetadata::new(source);

        assert!(!metadata.has_path());

        metadata.set_path("test.txt");
        assert!(metadata.has_path());
        assert_eq!(metadata.filename(), Some("test.txt"));

        metadata.clear_path();
        assert!(!metadata.has_path());
        assert_eq!(metadata.filename(), None);
    }

    #[test]
    fn test_serde_serialization() {
        let source = ContentSource::new();
        let metadata = ContentMetadata::with_path(source, PathBuf::from("test.json"));

        let serialized = serde_json::to_string(&metadata).unwrap();
        let deserialized: ContentMetadata = serde_json::from_str(&serialized).unwrap();

        assert_eq!(metadata, deserialized);
    }
}
