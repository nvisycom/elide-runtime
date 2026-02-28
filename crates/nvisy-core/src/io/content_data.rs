//! Content data structure for storing and managing content with metadata
//!
//! This module provides the [`ContentData`] struct for storing content data
//! along with its metadata and source information.

use std::fmt;
use std::ops::Deref;
use std::sync::OnceLock;

use bytes::Bytes;
use hipstr::HipStr;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::error::{Error, ErrorKind, Result};
use crate::path::ContentSource;

/// A wrapper around `Bytes` for content storage.
///
/// This struct wraps `bytes::Bytes` and provides additional methods
/// for text conversion. It's cheap to clone as `Bytes` uses reference
/// counting internally.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[derive(Serialize, Deserialize)]
#[serde(transparent)]
pub struct ContentBytes(Bytes);

impl ContentBytes {
    /// Creates a new `ContentBytes` from raw bytes.
    #[must_use]
    pub fn new(bytes: Bytes) -> Self {
        Self(bytes)
    }

    /// Returns the size of the content in bytes.
    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Returns `true` if the content is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Returns the content as a byte slice.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    /// Tries to return the content as a string slice.
    ///
    /// Returns `None` if the content is not valid UTF-8.
    #[must_use]
    pub fn as_str(&self) -> Option<&str> {
        std::str::from_utf8(&self.0).ok()
    }

    /// Converts to a `HipStr` if the content is valid UTF-8.
    ///
    /// # Errors
    ///
    /// Returns an error if the content is not valid UTF-8.
    pub fn as_hipstr(&self) -> Result<HipStr<'static>> {
        let s = std::str::from_utf8(&self.0).map_err(|e| {
            Error::new(ErrorKind::Serialization, format!("Invalid UTF-8: {e}"))
        })?;
        Ok(HipStr::from(s))
    }

    /// Returns the underlying `Bytes`.
    #[must_use]
    pub fn to_bytes(&self) -> Bytes {
        self.0.clone()
    }

    /// Consumes and returns the underlying `Bytes`.
    #[must_use]
    pub fn into_bytes(self) -> Bytes {
        self.0
    }

    /// Returns `true` if the content appears to be text.
    ///
    /// Uses a simple heuristic: checks if all bytes are ASCII printable
    /// or whitespace characters.
    #[must_use]
    pub fn is_likely_text(&self) -> bool {
        self.0
            .iter()
            .all(|&b| b.is_ascii_graphic() || b.is_ascii_whitespace())
    }
}

impl Deref for ContentBytes {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl AsRef<[u8]> for ContentBytes {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl From<&str> for ContentBytes {
    fn from(s: &str) -> Self {
        Self(Bytes::copy_from_slice(s.as_bytes()))
    }
}

impl From<String> for ContentBytes {
    fn from(s: String) -> Self {
        Self(Bytes::from(s))
    }
}

impl From<HipStr<'static>> for ContentBytes {
    fn from(s: HipStr<'static>) -> Self {
        Self(Bytes::copy_from_slice(s.as_bytes()))
    }
}

impl From<&[u8]> for ContentBytes {
    fn from(bytes: &[u8]) -> Self {
        Self(Bytes::copy_from_slice(bytes))
    }
}

impl From<Vec<u8>> for ContentBytes {
    fn from(vec: Vec<u8>) -> Self {
        Self(Bytes::from(vec))
    }
}

impl From<Bytes> for ContentBytes {
    fn from(bytes: Bytes) -> Self {
        Self(bytes)
    }
}

/// Content data with metadata and computed hashes.
///
/// This struct wraps [`ContentBytes`] and stores content data along with
/// metadata about its source and optional computed SHA256 hash.
/// It's designed to be cheap to clone using reference-counted types.
/// The SHA256 hash is lazily computed using `OnceLock` for lock-free
/// access after initialization.
#[derive(Debug, Serialize, Deserialize)]
pub struct ContentData {
    /// Unique identifier for the content source.
    pub content_source: ContentSource,
    /// The actual content data.
    data: ContentBytes,
    /// Lazily computed SHA256 hash of the content.
    #[serde(skip)]
    sha256_cache: OnceLock<Bytes>,
    /// Caller-supplied MIME type (e.g. from HTTP Content-Type header).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime: Option<String>,
    /// MIME type detected from magic bytes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detected_mime: Option<String>,
}

impl ContentData {
    /// Creates new content data from bytes.
    ///
    /// # Example
    ///
    /// ```
    /// use nvisy_core::{io::ContentData, path::ContentSource};
    /// use bytes::Bytes;
    ///
    /// let source = ContentSource::new();
    /// let data = Bytes::from("Hello, world!");
    /// let content = ContentData::new(source, data);
    ///
    /// assert_eq!(content.size(), 13);
    /// ```
    pub fn new(content_source: ContentSource, data: Bytes) -> Self {
        Self {
            content_source,
            data: ContentBytes::new(data),
            sha256_cache: OnceLock::new(),
            mime: None,
            detected_mime: None,
        }
    }

    /// Creates new content data from text.
    ///
    /// # Example
    ///
    /// ```
    /// use nvisy_core::{io::ContentData, path::ContentSource};
    ///
    /// let source = ContentSource::new();
    /// let content = ContentData::from_text(source, "Hello, world!");
    ///
    /// assert_eq!(content.as_str().unwrap(), "Hello, world!");
    /// ```
    pub fn from_text(content_source: ContentSource, text: impl Into<String>) -> Self {
        Self {
            content_source,
            data: ContentBytes::from(text.into()),
            sha256_cache: OnceLock::new(),
            mime: None,
            detected_mime: None,
        }
    }

    /// Creates content data with explicit `ContentBytes`.
    pub fn with_content_bytes(content_source: ContentSource, data: ContentBytes) -> Self {
        Self {
            content_source,
            data,
            sha256_cache: OnceLock::new(),
            mime: None,
            detected_mime: None,
        }
    }

    /// Set the caller-provided MIME type (builder pattern).
    #[must_use]
    pub fn with_content_type(mut self, mime: impl Into<String>) -> Self {
        self.mime = Some(mime.into());
        self
    }

    /// Get the best-available MIME type (provided takes precedence over detected).
    #[must_use]
    pub fn content_type(&self) -> Option<&str> {
        self.mime.as_deref().or(self.detected_mime.as_deref())
    }

    /// Detect the MIME type from magic bytes and store it.
    pub fn detect_mime(&mut self) {
        self.detected_mime = infer::get(self.data.as_bytes()).map(|t| t.mime_type().to_string());
    }

    /// Returns the size of the content in bytes.
    #[must_use]
    pub fn size(&self) -> usize {
        self.data.len()
    }

    /// Returns a pretty formatted size string.
    #[allow(clippy::cast_precision_loss)]
    #[must_use]
    pub fn get_pretty_size(&self) -> String {
        let bytes = self.size();
        match bytes {
            0..=1023 => format!("{bytes} B"),
            1024..=1_048_575 => format!("{:.1} KB", bytes as f64 / 1024.0),
            1_048_576..=1_073_741_823 => format!("{:.1} MB", bytes as f64 / 1_048_576.0),
            _ => format!("{:.1} GB", bytes as f64 / 1_073_741_824.0),
        }
    }

    /// Returns the content data as a byte slice.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        self.data.as_bytes()
    }

    /// Returns a reference to the underlying `ContentBytes`.
    #[must_use]
    pub fn content_bytes(&self) -> &ContentBytes {
        &self.data
    }

    /// Converts the content data to `Bytes`.
    #[must_use]
    pub fn to_bytes(&self) -> Bytes {
        self.data.to_bytes()
    }

    /// Consumes and converts into `Bytes`.
    #[must_use]
    pub fn into_bytes(self) -> Bytes {
        self.data.into_bytes()
    }

    /// Returns `true` if the content appears to be text.
    ///
    /// Uses a simple heuristic: checks if all bytes are ASCII printable
    /// or whitespace characters.
    #[must_use]
    pub fn is_likely_text(&self) -> bool {
        self.data.is_likely_text()
    }

    /// Tries to convert the content data to a UTF-8 string.
    ///
    /// # Errors
    ///
    /// Returns an error if the content data contains invalid UTF-8 sequences.
    pub fn as_string(&self) -> Result<String> {
        self.data.as_hipstr().map(|s| s.to_string())
    }

    /// Tries to convert the content data to a UTF-8 string slice.
    ///
    /// # Errors
    ///
    /// Returns an error if the content data contains invalid UTF-8 sequences.
    pub fn as_str(&self) -> Result<&str> {
        std::str::from_utf8(self.data.as_bytes()).map_err(|e| {
            Error::new(ErrorKind::Serialization, format!("Invalid UTF-8: {e}"))
        })
    }

    /// Converts to a `HipStr` if the content is valid UTF-8.
    ///
    /// # Errors
    ///
    /// Returns an error if the content is not valid UTF-8.
    pub fn as_hipstr(&self) -> Result<HipStr<'static>> {
        self.data.as_hipstr()
    }

    /// Computes SHA256 hash of the content.
    fn compute_sha256_internal(&self) -> Bytes {
        let mut hasher = Sha256::new();
        hasher.update(self.data.as_bytes());
        Bytes::from(hasher.finalize().to_vec())
    }

    /// Returns the SHA256 hash, computing it if not already done.
    #[must_use]
    pub fn sha256(&self) -> &Bytes {
        self.sha256_cache
            .get_or_init(|| self.compute_sha256_internal())
    }

    /// Returns the SHA256 hash as a hex string.
    #[must_use]
    pub fn sha256_hex(&self) -> String {
        hex::encode(self.sha256())
    }

    /// Verifies the content against a provided SHA256 hash.
    ///
    /// # Errors
    ///
    /// Returns an error if the computed hash does not match the expected hash.
    pub fn verify_sha256(&self, expected_hash: impl AsRef<[u8]>) -> Result<()> {
        let actual_hash = self.sha256();
        let expected = expected_hash.as_ref();

        if actual_hash.as_ref() == expected {
            Ok(())
        } else {
            Err(Error::new(ErrorKind::Validation, format!(
                "Hash mismatch: expected {}, got {}",
                hex::encode(expected),
                hex::encode(actual_hash)
            )))
        }
    }

    /// Returns a slice of the content data.
    ///
    /// # Errors
    ///
    /// Returns an error if the end index is beyond the content length
    /// or if start is greater than end.
    pub fn slice(&self, start: usize, end: usize) -> Result<Bytes> {
        let bytes = self.data.as_bytes();
        if end > bytes.len() {
            return Err(Error::new(ErrorKind::Validation, format!(
                "Slice end {} exceeds content length {}",
                end,
                bytes.len()
            )));
        }
        if start > end {
            return Err(Error::new(ErrorKind::Validation,
                format!("Slice start {start} is greater than end {end}")));
        }
        Ok(Bytes::copy_from_slice(&bytes[start..end]))
    }

    /// Returns `true` if the content is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
}

impl Clone for ContentData {
    fn clone(&self) -> Self {
        let new_lock = OnceLock::new();
        // Copy the computed hash if available
        if let Some(hash) = self.sha256_cache.get() {
            let _ = new_lock.set(hash.clone());
        }

        Self {
            content_source: self.content_source,
            data: self.data.clone(),
            sha256_cache: new_lock,
            mime: self.mime.clone(),
            detected_mime: self.detected_mime.clone(),
        }
    }
}

impl PartialEq for ContentData {
    fn eq(&self, other: &Self) -> bool {
        self.content_source == other.content_source
            && self.data == other.data
            && self.mime == other.mime
            && self.detected_mime == other.detected_mime
    }
}

impl Eq for ContentData {}

impl From<&str> for ContentData {
    fn from(s: &str) -> Self {
        let source = ContentSource::new();
        Self::from_text(source, s)
    }
}

impl From<String> for ContentData {
    fn from(s: String) -> Self {
        let source = ContentSource::new();
        Self::from_text(source, s)
    }
}

impl From<&[u8]> for ContentData {
    fn from(bytes: &[u8]) -> Self {
        let source = ContentSource::new();
        Self::new(source, Bytes::copy_from_slice(bytes))
    }
}

impl From<Vec<u8>> for ContentData {
    fn from(vec: Vec<u8>) -> Self {
        let source = ContentSource::new();
        Self::new(source, Bytes::from(vec))
    }
}

impl From<Bytes> for ContentData {
    fn from(bytes: Bytes) -> Self {
        let source = ContentSource::new();
        Self::new(source, bytes)
    }
}

impl From<HipStr<'static>> for ContentData {
    fn from(text: HipStr<'static>) -> Self {
        let source = ContentSource::new();
        Self::from_text(source, text.to_string())
    }
}

impl fmt::Display for ContentData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Ok(text) = self.as_str() {
            write!(f, "{text}")
        } else {
            write!(f, "[Binary data: {} bytes]", self.size())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_content_data_creation() {
        let source = ContentSource::new();
        let data = Bytes::from("Hello, world!");
        let content = ContentData::new(source, data);

        assert_eq!(content.content_source, source);
        assert_eq!(content.size(), 13);
        assert!(content.sha256_cache.get().is_none());
    }

    #[test]
    fn test_content_data_from_text() {
        let source = ContentSource::new();
        let content = ContentData::from_text(source, "Hello, world!");

        assert_eq!(content.as_str().unwrap(), "Hello, world!");
    }

    #[test]
    fn test_content_bytes_wrapper() {
        let bytes = ContentBytes::from("Hello");
        assert_eq!(bytes.as_str(), Some("Hello"));
        assert_eq!(bytes.len(), 5);
        assert!(!bytes.is_empty());
    }

    #[test]
    fn test_content_bytes_as_hipstr() {
        let bytes = ContentBytes::from("Hello, HipStr!");
        let hipstr = bytes.as_hipstr().unwrap();
        assert_eq!(hipstr.as_str(), "Hello, HipStr!");

        // Test with invalid UTF-8
        let invalid = ContentBytes::from(vec![0xFF, 0xFE]);
        assert!(invalid.as_hipstr().is_err());
    }

    #[test]
    fn test_content_bytes_binary() {
        let binary = ContentBytes::from(vec![0xFF, 0xFE]);
        assert_eq!(binary.len(), 2);
        assert!(binary.as_str().is_none());
        assert!(!binary.is_likely_text());
    }

    #[test]
    fn test_size_methods() {
        let content = ContentData::from("Hello");
        assert_eq!(content.size(), 5);

        let pretty_size = content.get_pretty_size();
        assert!(!pretty_size.is_empty());
    }

    #[test]
    fn test_sha256_computation() {
        let content = ContentData::from("Hello, world!");
        let hash = content.sha256();

        assert!(content.sha256_cache.get().is_some());
        assert_eq!(hash.len(), 32);

        let hash2 = content.sha256();
        assert_eq!(hash, hash2);
    }

    #[test]
    fn test_sha256_verification() {
        let content = ContentData::from("Hello, world!");
        let hash = content.sha256().clone();

        assert!(content.verify_sha256(&hash).is_ok());

        let wrong_hash = vec![0u8; 32];
        assert!(content.verify_sha256(&wrong_hash).is_err());
    }

    #[test]
    fn test_string_conversion() {
        let content = ContentData::from("Hello, world!");
        assert_eq!(content.as_string().unwrap(), "Hello, world!");
        assert_eq!(content.as_str().unwrap(), "Hello, world!");

        let binary_content = ContentData::from(vec![0xFF, 0xFE, 0xFD]);
        assert!(binary_content.as_string().is_err());
        assert!(binary_content.as_str().is_err());
    }

    #[test]
    fn test_as_hipstr() {
        let content = ContentData::from("Hello, HipStr!");
        let hipstr = content.as_hipstr().unwrap();
        assert_eq!(hipstr.as_str(), "Hello, HipStr!");

        let binary_content = ContentData::from(vec![0xFF, 0xFE]);
        assert!(binary_content.as_hipstr().is_err());
    }

    #[test]
    fn test_is_likely_text() {
        let text_content = ContentData::from("Hello, world!");
        assert!(text_content.is_likely_text());

        let binary_content = ContentData::from(vec![0xFF, 0xFE, 0xFD]);
        assert!(!binary_content.is_likely_text());
    }

    #[test]
    fn test_slice() {
        let content = ContentData::from("Hello, world!");

        let slice = content.slice(0, 5).unwrap();
        assert_eq!(slice, Bytes::from("Hello"));

        let slice = content.slice(7, 12).unwrap();
        assert_eq!(slice, Bytes::from("world"));

        assert!(content.slice(0, 100).is_err());
        assert!(content.slice(10, 5).is_err());
    }

    #[test]
    fn test_from_conversions() {
        let from_str = ContentData::from("test");
        let from_string = ContentData::from("test".to_string());
        let from_bytes = ContentData::from(b"test".as_slice());
        let from_vec = ContentData::from(b"test".to_vec());
        let from_bytes_type = ContentData::from(Bytes::from("test"));

        assert_eq!(from_str.as_str().unwrap(), "test");
        assert_eq!(from_string.as_str().unwrap(), "test");
        assert_eq!(from_bytes.as_str().unwrap(), "test");
        assert_eq!(from_vec.as_str().unwrap(), "test");
        assert_eq!(from_bytes_type.as_str().unwrap(), "test");
    }

    #[test]
    fn test_display() {
        let text_content = ContentData::from("Hello");
        assert_eq!(format!("{text_content}"), "Hello");

        let binary_content = ContentData::from(vec![0xFF, 0xFE]);
        assert!(format!("{binary_content}").contains("Binary data"));
    }

    #[test]
    fn test_cloning_preserves_hash() {
        let original = ContentData::from("Hello, world!");
        let _ = original.sha256();

        let cloned = original.clone();

        assert!(original.sha256_cache.get().is_some());
        assert!(cloned.sha256_cache.get().is_some());
        assert_eq!(original.sha256(), cloned.sha256());
    }

    #[test]
    fn test_cloning_is_cheap() {
        let original = ContentData::from("Hello, world!");
        let cloned = original.clone();

        assert_eq!(original, cloned);
    }

    #[test]
    fn test_into_bytes() {
        let content = ContentData::from("Hello, world!");
        let bytes = content.into_bytes();
        assert_eq!(bytes, Bytes::from("Hello, world!"));
    }

    #[test]
    fn test_empty_content() {
        let content = ContentData::from("");
        assert!(content.is_empty());
        assert_eq!(content.size(), 0);
    }

    #[test]
    fn test_to_bytes() {
        let text_content = ContentData::from_text(ContentSource::new(), "Hello");
        let bytes = text_content.to_bytes();
        assert_eq!(bytes.as_ref(), b"Hello");

        let binary_content = ContentData::new(ContentSource::new(), Bytes::from("World"));
        let bytes = binary_content.to_bytes();
        assert_eq!(bytes.as_ref(), b"World");
    }

    #[test]
    fn test_from_hipstr() {
        let hipstr = HipStr::from("Hello from HipStr");
        let content = ContentData::from(hipstr);
        assert_eq!(content.as_str().unwrap(), "Hello from HipStr");
    }

    #[test]
    fn test_content_bytes_deref() {
        let bytes = ContentBytes::from("Hello");
        assert_eq!(&*bytes, b"Hello");
        assert_eq!(bytes.as_ref(), b"Hello");
    }
}
