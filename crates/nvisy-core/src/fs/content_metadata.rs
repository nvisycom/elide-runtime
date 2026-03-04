//! Content metadata for filesystem operations
//!
//! This module provides the [`ContentMetadata`] struct for handling metadata
//! about content files, including paths and source tracking.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Metadata associated with content files
///
/// This struct stores metadata about content including its file path
/// and arbitrary key-value pairs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContentMetadata {
    /// Optional path to the source file
    pub source_path: Option<PathBuf>,
    /// Arbitrary key-value metadata associated with this content.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Map<String, serde_json::Value>>,
}

impl ContentMetadata {
    /// Create new empty content metadata.
    ///
    /// # Example
    ///
    /// ```
    /// use nvisy_core::fs::ContentMetadata;
    ///
    /// let metadata = ContentMetadata::new();
    /// ```
    #[must_use]
    pub fn new() -> Self {
        Self {
            source_path: None,
            metadata: None,
        }
    }

    /// Create content metadata with a file path.
    ///
    /// # Example
    ///
    /// ```
    /// use nvisy_core::fs::ContentMetadata;
    /// use std::path::PathBuf;
    ///
    /// let metadata = ContentMetadata::with_path(PathBuf::from("document.pdf"));
    /// assert_eq!(metadata.file_extension(), Some("pdf"));
    /// ```
    pub fn with_path(path: impl Into<PathBuf>) -> Self {
        Self {
            source_path: Some(path.into()),
            metadata: None,
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

    /// Get the extra metadata map, if any.
    #[must_use]
    pub fn extra(&self) -> Option<&serde_json::Map<String, serde_json::Value>> {
        self.metadata.as_ref()
    }

    /// Get a single value from the extra metadata map.
    #[must_use]
    pub fn get_extra(&self, key: &str) -> Option<&serde_json::Value> {
        self.metadata.as_ref().and_then(|m| m.get(key))
    }

    /// Insert a key-value pair into the extra metadata map,
    /// creating the map if it doesn't exist yet.
    pub fn set_extra(&mut self, key: impl Into<String>, value: serde_json::Value) {
        self.metadata
            .get_or_insert_with(serde_json::Map::new)
            .insert(key.into(), value);
    }

    /// Remove a key from the extra metadata map.
    /// Returns the removed value if the key existed.
    pub fn remove_extra(&mut self, key: &str) -> Option<serde_json::Value> {
        self.metadata.as_mut().and_then(|m| m.remove(key))
    }
}

impl Default for ContentMetadata {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_extension_detection() {
        let metadata = ContentMetadata::with_path(PathBuf::from("document.pdf"));

        assert_eq!(metadata.file_extension(), Some("pdf"));
    }

    #[test]
    fn test_metadata_filename() {
        let metadata = ContentMetadata::with_path(PathBuf::from("/path/to/file.txt"));

        assert_eq!(metadata.filename(), Some("file.txt"));
    }

    #[test]
    fn test_metadata_parent_directory() {
        let metadata = ContentMetadata::with_path(PathBuf::from("/path/to/file.txt"));

        assert_eq!(metadata.parent_directory(), Some(Path::new("/path/to")));
    }

    #[test]
    fn test_path_operations() {
        let mut metadata = ContentMetadata::new();

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
        let metadata = ContentMetadata::with_path(PathBuf::from("test.json"));

        let serialized = serde_json::to_string(&metadata).unwrap();
        let deserialized: ContentMetadata = serde_json::from_str(&serialized).unwrap();

        assert_eq!(metadata, deserialized);
    }

    #[test]
    fn test_extra_metadata() {
        let mut metadata = ContentMetadata::new();
        assert!(metadata.extra().is_none());
        assert!(metadata.get_extra("key").is_none());

        metadata.set_extra("lang", serde_json::Value::String("en".into()));
        assert_eq!(
            metadata.get_extra("lang"),
            Some(&serde_json::Value::String("en".into()))
        );
        assert!(metadata.extra().is_some());

        let removed = metadata.remove_extra("lang");
        assert_eq!(removed, Some(serde_json::Value::String("en".into())));
        assert_eq!(metadata.get_extra("lang"), None);
    }

    #[test]
    fn test_extra_metadata_serialization() {
        let mut metadata = ContentMetadata::with_path("doc.pdf");
        metadata.set_extra("pages", serde_json::json!(42));

        let json = serde_json::to_string(&metadata).unwrap();
        let deserialized: ContentMetadata = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.get_extra("pages"), Some(&serde_json::json!(42)));
        assert_eq!(deserialized.filename(), Some("doc.pdf"));
    }
}
