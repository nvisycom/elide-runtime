//! Descriptive metadata for content: MIME type, filename, source path,
//! and arbitrary key-value pairs.
//!
//! `ContentMetadata` is persisted separately from the raw bytes so that
//! information that cannot be recovered from magic-byte detection (e.g.
//! `text/plain` MIME type, original filename) survives a registry
//! round-trip.

use std::path::{Path, PathBuf};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::entity::{Annotation, LabelAnnotation};
use crate::modality::{Audio, Image, Tabular, Text};

/// Descriptive metadata associated with content.
///
/// Stored alongside (but separate from) the raw content bytes. Carries
/// the caller-supplied MIME type, auto-detected MIME type, original
/// filename, source path, and arbitrary key-value pairs.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
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
    /// Used by [`CodecRegistry`] for extension-based format
    /// resolution.
    ///
    /// [`CodecRegistry`]: nvisy_codec::CodecRegistry
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filename: Option<PathBuf>,
    /// Content size in bytes, persisted at upload.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<u64>,
    /// SHA-256 hex digest, persisted at upload.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
    /// Arbitrary key-value pairs associated with this content. The
    /// name avoids the self-referential `ContentMetadata::metadata`
    /// and matches the existing accessors ([`extra`],
    /// [`get_extra`], [`set_extra`], [`remove_extra`]).
    ///
    /// [`extra`]: Self::extra
    /// [`get_extra`]: Self::get_extra
    /// [`set_extra`]: Self::set_extra
    /// [`remove_extra`]: Self::remove_extra
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extra: Option<serde_json::Map<String, serde_json::Value>>,
    /// Pre-identified regions and classification labels for this
    /// content, bucketed per modality. The importer routes each
    /// bucket to its matching `Document<M>` envelope; labels
    /// (modality-agnostic) propagate to every envelope spawned from
    /// the source.
    #[serde(default, skip_serializing_if = "AnyAnnotations::is_empty")]
    pub annotations: AnyAnnotations,
}

/// Per-modality buckets of user-supplied annotations on a piece of
/// content.
///
/// Each modality-typed [`Annotation<M>`] targets a `Document<M>`
/// envelope of the same modality; document-level
/// [`LabelAnnotation`]s apply to every envelope spawned from the
/// source.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AnyAnnotations {
    /// Annotations targeting text-modality content.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub text: Vec<Annotation<Text>>,
    /// Annotations targeting tabular-modality content.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tabular: Vec<Annotation<Tabular>>,
    /// Annotations targeting image-modality content.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub image: Vec<Annotation<Image>>,
    /// Annotations targeting audio-modality content.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub audio: Vec<Annotation<Audio>>,
    /// Document-level classification labels.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub labels: Vec<LabelAnnotation>,
}

impl AnyAnnotations {
    /// `true` when every bucket is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
            && self.tabular.is_empty()
            && self.image.is_empty()
            && self.audio.is_empty()
            && self.labels.is_empty()
    }
}

impl ContentMetadata {
    /// Create new empty content metadata.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
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
            extra: None,
            annotations: AnyAnnotations::default(),
        }
    }

    /// Set annotations (builder pattern).
    #[must_use]
    pub fn with_annotations(mut self, annotations: AnyAnnotations) -> Self {
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

    /// Get the file extension from the source path, if available.
    #[must_use]
    pub fn file_extension(&self) -> Option<&str> {
        self.source_path
            .as_ref()
            .and_then(|path| path.extension())
            .and_then(|ext| ext.to_str())
    }

    /// Get the full path if available
    #[must_use]
    pub fn path(&self) -> Option<&Path> {
        self.source_path.as_deref()
    }

    /// Get a single value from the extra metadata map.
    #[must_use]
    pub fn get_extra(&self, key: &str) -> Option<&serde_json::Value> {
        self.extra.as_ref().and_then(|m| m.get(key))
    }

    /// Insert a key-value pair into the extra metadata map,
    /// creating the map if it doesn't exist yet.
    pub fn set_extra(&mut self, key: impl Into<String>, value: serde_json::Value) {
        self.extra
            .get_or_insert_with(serde_json::Map::new)
            .insert(key.into(), value);
    }

    /// Remove a key from the extra metadata map. Returns the removed
    /// value if the key existed.
    pub fn remove_extra(&mut self, key: &str) -> Option<serde_json::Value> {
        self.extra.as_mut().and_then(|m| m.remove(key))
    }
}
