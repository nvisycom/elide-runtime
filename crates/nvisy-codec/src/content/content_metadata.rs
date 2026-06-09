//! Two-layer content metadata:
//!
//! - [`ContentDescriptor`] holds caller-supplied descriptive bits
//!   (filename, MIME hint, source path, arbitrary extras). All
//!   optional — the caller might or might not have any of them.
//! - [`ContentDigest`] holds facts the registry computes by looking
//!   at the bytes (size, sha256, sniffed MIME). Required fields are
//!   actually required.
//! - [`ContentRecord`] bundles a descriptor with a digest. This is
//!   what the registry persists and what read sites get back.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Caller-supplied descriptive metadata for an upload.
///
/// Built before the bytes have been written to the registry, so
/// every field is optional — the caller knows whatever they know.
/// The registry's `register_content` consumes this alongside the
/// bytes to produce a [`ContentRecord`].
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ContentDescriptor {
    /// Optional path to the source file.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_path: Option<PathBuf>,
    /// MIME type supplied by the caller (e.g. `"text/plain"` from
    /// an HTTP `Content-Type` header or an explicit API call).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_type: Option<String>,
    /// Original filename, if known. Used by `CodecRegistry` for
    /// extension-based format resolution.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filename: Option<PathBuf>,
    /// Arbitrary key-value pairs the caller wants associated with
    /// this content. Read by policy conditions
    /// (`Condition::Metadata { key, value }`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extra: Option<serde_json::Map<String, serde_json::Value>>,
}

impl ContentDescriptor {
    /// Create an empty descriptor.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a descriptor with a source file path.
    pub fn with_path(path: impl Into<PathBuf>) -> Self {
        Self {
            source_path: Some(path.into()),
            ..Self::default()
        }
    }

    /// Set the caller-supplied MIME type (builder pattern).
    #[must_use]
    pub fn with_content_type(mut self, mime: impl Into<String>) -> Self {
        self.content_type = Some(mime.into());
        self
    }

    /// Set the original filename (builder pattern).
    #[must_use]
    pub fn with_filename(mut self, name: impl Into<PathBuf>) -> Self {
        self.filename = Some(name.into());
        self
    }

    /// Get the file extension from the source path, if available.
    #[must_use]
    pub fn file_extension(&self) -> Option<&str> {
        self.source_path
            .as_ref()
            .and_then(|path| path.extension())
            .and_then(|ext| ext.to_str())
    }

    /// Get the full path if available.
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

/// Byte-derived facts about a piece of content.
///
/// Computed by `Registry::register_content` after the bytes are in
/// hand. Required fields (`size`, `sha256`) are unconditional;
/// `detected_content_type` is `Option` because magic-byte sniffing
/// may legitimately fail (e.g. plain text).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContentDigest {
    /// Size in bytes.
    pub size: u64,
    /// SHA-256 hex digest of the raw bytes.
    pub sha256: String,
    /// MIME type sniffed from the bytes, if magic-byte detection
    /// produced a result.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detected_content_type: Option<String>,
}

/// Persisted, post-registration view of a piece of content.
///
/// Returned by registry read endpoints (`list_content_with_record`,
/// `read_content`). The [`ContentDescriptor`] half is whatever the
/// caller supplied at upload; the [`ContentDigest`] half is what
/// the registry computed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContentRecord {
    /// Caller-supplied descriptor (filename, MIME hint, extras).
    pub descriptor: ContentDescriptor,
    /// Registry-computed digest (size, sha256, detected MIME).
    pub digest: ContentDigest,
}

impl ContentRecord {
    /// Best-available MIME type: caller-supplied takes priority
    /// over sniffed.
    #[must_use]
    pub fn content_type(&self) -> Option<&str> {
        self.descriptor
            .content_type
            .as_deref()
            .or(self.digest.detected_content_type.as_deref())
    }

    /// Original filename from the descriptor.
    #[must_use]
    pub fn filename(&self) -> Option<&Path> {
        self.descriptor.filename.as_deref()
    }

    /// Original filename rendered as a UTF-8 string. Non-UTF-8 byte
    /// sequences in the path are replaced with U+FFFD (lossy
    /// conversion). Use [`filename`] when you need the raw `&Path`.
    ///
    /// [`filename`]: Self::filename
    #[must_use]
    pub fn filename_lossy(&self) -> Option<String> {
        self.filename().map(|p| p.to_string_lossy().into_owned())
    }

    /// File extension from the descriptor's source path.
    #[must_use]
    pub fn file_extension(&self) -> Option<&str> {
        self.descriptor.file_extension()
    }
}
