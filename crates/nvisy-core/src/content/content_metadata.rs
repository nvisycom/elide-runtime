//! Descriptive metadata for content: MIME type, filename, source path,
//! and arbitrary key-value pairs.
//!
//! `ContentMetadata` is persisted separately from the raw bytes so that
//! information that cannot be recovered from magic-byte detection (e.g.
//! `text/plain` MIME type, original filename) survives a registry
//! round-trip.

use std::path::{Path, PathBuf};

use nvisy_ontology::entity::Annotations;
use serde::{Deserialize, Serialize};

use crate::media::DocumentType;

/// Descriptive metadata associated with content.
///
/// Stored alongside (but separate from) the raw content bytes. Carries
/// the caller-supplied MIME type, auto-detected MIME type, original
/// filename, source path, and arbitrary key-value pairs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContentMetadata {
    /// Optional path to the source file.
    pub source_path: Option<PathBuf>,
    /// MIME type supplied by the caller (e.g. `"text/plain"`, from an
    /// HTTP `Content-Type` header or explicit API call).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_type: Option<String>,
    /// MIME type detected from magic bytes (computed eagerly on upload).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detected_content_type: Option<String>,
    /// Original filename, if known (e.g. from upload or file path).
    ///
    /// Used for extension-based format refinement (e.g. `.log` →
    /// `TextFormat::Log`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filename: Option<PathBuf>,
    /// Content size in bytes, persisted at upload.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<u64>,
    /// SHA-256 hex digest, persisted at upload.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
    /// Arbitrary key-value metadata associated with this content.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Map<String, serde_json::Value>>,
    /// Pre-identified regions and classification labels for this content.
    #[serde(default, skip_serializing_if = "Annotations::is_empty")]
    pub annotations: Annotations,
}

impl ContentMetadata {
    /// Create new empty content metadata.
    #[must_use]
    pub fn new() -> Self {
        Self {
            source_path: None,
            content_type: None,
            detected_content_type: None,
            filename: None,
            size: None,
            sha256: None,
            metadata: None,
            annotations: Annotations::new(),
        }
    }

    /// Create content metadata with a source file path.
    pub fn with_path(path: impl Into<PathBuf>) -> Self {
        Self {
            source_path: Some(path.into()),
            content_type: None,
            detected_content_type: None,
            filename: None,
            size: None,
            sha256: None,
            metadata: None,
            annotations: Annotations::new(),
        }
    }

    /// Set annotations (builder pattern).
    #[must_use]
    pub fn with_annotations(mut self, annotations: Annotations) -> Self {
        self.annotations = annotations;
        self
    }

    /// Set the caller-supplied MIME type (builder pattern).
    #[must_use]
    pub fn with_content_type(mut self, mime: impl Into<String>) -> Self {
        self.content_type = Some(mime.into());
        self
    }

    /// Set the auto-detected MIME type (builder pattern).
    #[must_use]
    pub fn with_detected_content_type(mut self, mime: impl Into<String>) -> Self {
        self.detected_content_type = Some(mime.into());
        self
    }

    /// Set the original filename (builder pattern).
    #[must_use]
    pub fn with_filename(mut self, name: impl Into<PathBuf>) -> Self {
        self.filename = Some(name.into());
        self
    }

    /// Best-available MIME type: supplied takes priority over detected.
    #[must_use]
    pub fn content_type(&self) -> Option<&str> {
        self.content_type
            .as_deref()
            .or(self.detected_content_type.as_deref())
    }

    /// Infer the [`DocumentType`] from MIME type and filename extension.
    ///
    /// Priority: supplied MIME > detected MIME > filename extension.
    /// When the base type is `Text` and the extension refines it
    /// (e.g. `.log`), the refined variant wins.
    #[must_use]
    pub fn infer_document_type(&self) -> Option<DocumentType> {
        let from_supplied = self
            .content_type
            .as_deref()
            .and_then(DocumentType::from_mime);
        let from_detected = self
            .detected_content_type
            .as_deref()
            .and_then(DocumentType::from_mime);
        let from_ext = self
            .filename
            .as_ref()
            .and_then(|f| f.extension())
            .and_then(DocumentType::from_extension);

        let result = from_supplied.or(from_detected);
        match (result, from_ext) {
            (Some(DocumentType::Text(_)), Some(refined @ DocumentType::Text(_))) => Some(refined),
            _ => result.or(from_ext),
        }
    }

    /// Get the file extension from the source path, if available.
    #[must_use]
    pub fn file_extension(&self) -> Option<&str> {
        self.source_path
            .as_ref()
            .and_then(|path| path.extension())
            .and_then(|ext| ext.to_str())
    }

    /// Get the filename from the source path, if available.
    #[must_use]
    pub fn filename_from_path(&self) -> Option<&str> {
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

        assert_eq!(metadata.filename_from_path(), Some("file.txt"));
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
        assert_eq!(metadata.filename_from_path(), Some("test.txt"));

        metadata.clear_path();
        assert!(!metadata.has_path());
        assert_eq!(metadata.filename_from_path(), None);
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

        assert_eq!(
            deserialized.get_extra("pages"),
            Some(&serde_json::json!(42))
        );
        assert_eq!(deserialized.filename_from_path(), Some("doc.pdf"));
    }
}
