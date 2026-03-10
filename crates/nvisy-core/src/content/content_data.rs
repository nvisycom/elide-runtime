//! Content data structure for storing and managing content with metadata
//!
//! This module provides the [`ContentData`] struct for storing content data
//! along with its metadata and source information.

use std::fmt;

use bytes::Bytes;
use hipstr::HipStr;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::content_bytes::ContentBytes;
use crate::error::{Error, ErrorKind, Result};
use super::ContentSource;
use crate::media::DocumentType;

/// Content data with metadata and computed hashes.
///
/// This struct wraps [`ContentBytes`] and stores content data along with
/// metadata about its source.
/// It's designed to be cheap to clone using reference-counted types.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContentData {
    /// Unique identifier for the content source.
    pub content_source: ContentSource,
    /// The actual content data.
    data: ContentBytes,
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
    /// use nvisy_core::content::{ContentData, ContentSource};
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
            data: ContentBytes::from(data),

            mime: None,
            detected_mime: None,
        }
    }

    /// Creates new content data from text.
    ///
    /// # Example
    ///
    /// ```
    /// use nvisy_core::content::{ContentData, ContentSource};
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

            mime: None,
            detected_mime: None,
        }
    }

    /// Creates content data with explicit `ContentBytes`.
    pub fn with_content_bytes(content_source: ContentSource, data: ContentBytes) -> Self {
        Self {
            content_source,
            data,

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

    /// Detect the MIME type from magic bytes and cache the result.
    ///
    /// The detected type is stored in `detected_mime` and returned by
    /// [`content_type`](Self::content_type) when no explicit MIME is set.
    pub fn detect_mime(&mut self) -> Option<&str> {
        self.detected_mime = infer::get(self.data.as_bytes()).map(|t| t.mime_type().to_owned());
        self.detected_mime.as_deref()
    }

    /// Detect the [`DocumentType`] from the best-available MIME type,
    /// caching the detected MIME for future calls.
    ///
    /// Calls [`detect_mime`](Self::detect_mime) if no MIME type is
    /// available yet, then maps the result via [`DocumentType::from_mime`].
    pub fn document_type(&mut self) -> Option<DocumentType> {
        if self.content_type().is_none() {
            self.detect_mime();
        }
        self.content_type().and_then(DocumentType::from_mime)
    }

    /// Infer the [`DocumentType`] without mutating or caching.
    ///
    /// Uses the caller-supplied or previously detected MIME if available,
    /// otherwise falls back to magic-byte sniffing on the raw content.
    #[must_use]
    pub fn infer_document_type(&self) -> Option<DocumentType> {
        self.content_type()
            .and_then(DocumentType::from_mime)
            .or_else(|| {
                let kind = infer::get(self.data.as_bytes())?;
                DocumentType::from_mime(kind.mime_type())
            })
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
        Bytes::copy_from_slice(self.data.as_bytes())
    }

    /// Consumes and converts into `Bytes`.
    #[must_use]
    pub fn into_bytes(self) -> Bytes {
        self.data.into_inner()
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
        std::str::from_utf8(self.data.as_bytes())
            .map_err(|e| Error::new(ErrorKind::Serialization, format!("Invalid UTF-8: {e}")))
    }

    /// Converts to a `HipStr` if the content is valid UTF-8.
    ///
    /// # Errors
    ///
    /// Returns an error if the content is not valid UTF-8.
    pub fn as_hipstr(&self) -> Result<HipStr<'static>> {
        self.data.as_hipstr()
    }

    /// Computes and returns the SHA256 hash of the content.
    #[must_use]
    pub fn sha256(&self) -> Bytes {
        let mut hasher = Sha256::new();
        hasher.update(self.data.as_bytes());
        Bytes::from(hasher.finalize().to_vec())
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
            Err(Error::new(
                ErrorKind::Validation,
                format!(
                    "Hash mismatch: expected {}, got {}",
                    hex::encode(expected),
                    hex::encode(actual_hash)
                ),
            ))
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
            return Err(Error::new(
                ErrorKind::Validation,
                format!("Slice end {} exceeds content length {}", end, bytes.len()),
            ));
        }
        if start > end {
            return Err(Error::new(
                ErrorKind::Validation,
                format!("Slice start {start} is greater than end {end}"),
            ));
        }
        Ok(Bytes::copy_from_slice(&bytes[start..end]))
    }

    /// Returns `true` if the content is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
}

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
    }

    #[test]
    fn test_content_data_from_text() {
        let source = ContentSource::new();
        let content = ContentData::from_text(source, "Hello, world!");

        assert_eq!(content.as_str().unwrap(), "Hello, world!");
    }

    #[test]
    fn test_sha256_computation() {
        let content = ContentData::from("Hello, world!");
        let hash = content.sha256();

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
    fn test_as_hipstr() {
        let content = ContentData::from("Hello, HipStr!");
        let hipstr = content.as_hipstr().unwrap();
        assert_eq!(hipstr.as_str(), "Hello, HipStr!");

        let binary_content = ContentData::from(vec![0xFF, 0xFE]);
        assert!(binary_content.as_hipstr().is_err());
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
        let cloned = original.clone();

        assert_eq!(original.sha256(), cloned.sha256());
    }

    #[test]
    fn test_from_hipstr() {
        let hipstr = HipStr::from("Hello from HipStr");
        let content = ContentData::from(hipstr);
        assert_eq!(content.as_str().unwrap(), "Hello from HipStr");
    }

    #[test]
    fn test_detect_mime_png() {
        let png = vec![
            0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, // PNG signature
            0x00, 0x00, 0x00, 0x0D, // IHDR length
            0x49, 0x48, 0x44, 0x52, // IHDR
        ];
        let mut content = ContentData::from(png);
        assert_eq!(content.detect_mime(), Some("image/png"));
        assert_eq!(content.detected_mime.as_deref(), Some("image/png"));
    }

    #[test]
    fn test_detect_mime_unknown() {
        let mut content = ContentData::from("hello world");
        assert_eq!(content.detect_mime(), None);
        assert_eq!(content.detected_mime, None);
    }

    #[test]
    fn test_document_type_from_magic_bytes() {
        use crate::media::ImageFormat;

        let png = vec![
            0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48,
            0x44, 0x52,
        ];
        let mut content = ContentData::from(png);
        assert_eq!(
            content.document_type(),
            Some(DocumentType::Image(ImageFormat::Png)),
        );
    }

    #[test]
    fn test_document_type_prefers_explicit_mime() {
        let mut content =
            ContentData::from("not really json").with_content_type("application/json");
        assert_eq!(
            content.document_type(),
            Some(DocumentType::Text(crate::media::TextFormat::Json)),
        );
    }

    #[test]
    fn test_content_type_precedence() {
        let png = vec![
            0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48,
            0x44, 0x52,
        ];
        let mut content = ContentData::from(png).with_content_type("image/jpeg");
        // Explicit MIME takes precedence over detected.
        content.detect_mime();
        assert_eq!(content.content_type(), Some("image/jpeg"));
    }

    #[test]
    fn test_infer_document_type_formats() {
        use crate::media::{AudioFormat, ImageFormat};

        let jpeg = ContentData::from(vec![0xFF, 0xD8, 0xFF, 0xE0]);
        assert_eq!(
            jpeg.infer_document_type(),
            Some(DocumentType::Image(ImageFormat::Jpeg)),
        );

        let mut wav = [0u8; 12];
        wav[..4].copy_from_slice(b"RIFF");
        wav[8..12].copy_from_slice(b"WAVE");
        let wav = ContentData::from(wav.to_vec());
        assert_eq!(
            wav.infer_document_type(),
            Some(DocumentType::Audio(AudioFormat::Wav)),
        );

        let mp3 = ContentData::from(vec![0x49, 0x44, 0x33]);
        assert_eq!(
            mp3.infer_document_type(),
            Some(DocumentType::Audio(AudioFormat::Mp3)),
        );

        let pdf = ContentData::from(b"%PDF-1.4".to_vec());
        assert_eq!(pdf.infer_document_type(), Some(DocumentType::Pdf));
    }

    #[test]
    fn test_infer_document_type_unknown() {
        assert_eq!(ContentData::from("hello world").infer_document_type(), None);
        assert_eq!(ContentData::from("").infer_document_type(), None);
    }

    #[test]
    fn test_infer_document_type_respects_explicit_mime() {
        let content = ContentData::from("not really json").with_content_type("application/json");
        assert_eq!(
            content.infer_document_type(),
            Some(DocumentType::Text(crate::media::TextFormat::Json)),
        );
    }
}
