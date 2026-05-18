//! Raw content bytes with source identity.
//!
//! [`ContentData`] is the pure data half of the content model. It holds
//! the raw bytes and a [`ContentSource`] identifier. All descriptive
//! attributes (MIME type, filename, arbitrary metadata) live on
//! [`ContentMetadata`](super::ContentMetadata).

use std::{fmt, str};

use bytes::Bytes;
use hipstr::HipStr;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::ContentSource;
use crate::error::{Error, ErrorKind, Result};

/// Raw content bytes with source identity.
///
/// This is the data-only half of the content model — it does not carry
/// MIME type, filename, or other descriptive metadata. Pair with
/// [`ContentMetadata`](super::ContentMetadata) via
/// [`Content`](super::Content) for a complete representation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContentData {
    /// Unique identifier for the content source.
    pub content_source: ContentSource,
    /// The actual content bytes.
    data: Bytes,
}

impl ContentData {
    /// Creates content data from raw bytes.
    pub fn new(content_source: ContentSource, data: Bytes) -> Self {
        Self {
            content_source,
            data,
        }
    }

    /// Creates content data from a text string.
    pub fn from_text(content_source: ContentSource, text: impl Into<String>) -> Self {
        Self {
            content_source,
            data: Bytes::from(text.into().into_bytes()),
        }
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
        &self.data
    }

    /// Converts the content data to `Bytes`.
    #[must_use]
    pub fn to_bytes(&self) -> Bytes {
        self.data.clone()
    }

    /// Consumes and converts into `Bytes`.
    #[must_use]
    pub fn into_bytes(self) -> Bytes {
        self.data
    }

    /// Returns `true` if the content appears to be text.
    ///
    /// Checks that the content is valid UTF-8 and contains no control
    /// characters other than common whitespace (tab, newline, carriage
    /// return).
    #[must_use]
    pub fn is_likely_text(&self) -> bool {
        let Ok(s) = str::from_utf8(&self.data) else {
            return false;
        };
        s.chars()
            .all(|c| !c.is_control() || matches!(c, '\t' | '\n' | '\r'))
    }

    /// Tries to convert the content data to a UTF-8 string slice.
    ///
    /// # Errors
    ///
    /// Returns an error if the content data contains invalid UTF-8 sequences.
    pub fn as_str(&self) -> Result<&str> {
        str::from_utf8(&self.data)
            .map_err(|e| Error::new(ErrorKind::Serialization, format!("Invalid UTF-8: {e}")))
    }

    /// Tries to convert the content data to an owned string.
    ///
    /// # Errors
    ///
    /// Returns an error if the content data contains invalid UTF-8 sequences.
    pub fn as_string(&self) -> Result<String> {
        self.as_str().map(String::from)
    }

    /// Converts to a `HipStr` if the content is valid UTF-8.
    ///
    /// # Errors
    ///
    /// Returns an error if the content is not valid UTF-8.
    pub fn as_hipstr(&self) -> Result<HipStr<'static>> {
        let s = str::from_utf8(&self.data)
            .map_err(|e| Error::new(ErrorKind::Serialization, format!("Invalid UTF-8: {e}")))?;
        Ok(HipStr::from(s))
    }

    /// Computes and returns the SHA256 hash of the content.
    #[must_use]
    pub fn sha256(&self) -> Bytes {
        let mut hasher = Sha256::new();
        hasher.update(&self.data);
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
        let bytes = &self.data;
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

    /// Detect MIME type from the raw bytes using magic-byte signatures.
    ///
    /// Returns `None` for content with no recognizable magic bytes
    /// (e.g. plain text). Use this to populate
    /// [`ContentMetadata::detected_content_type`](super::ContentMetadata::detected_content_type).
    #[must_use]
    pub fn detect_mime(&self) -> Option<String> {
        infer::get(&self.data).map(|t| t.mime_type().to_owned())
    }
}

impl From<&str> for ContentData {
    fn from(s: &str) -> Self {
        Self::from_text(ContentSource::new(), s)
    }
}

impl From<String> for ContentData {
    fn from(s: String) -> Self {
        Self::from_text(ContentSource::new(), s)
    }
}

impl From<&[u8]> for ContentData {
    fn from(bytes: &[u8]) -> Self {
        Self::new(ContentSource::new(), Bytes::copy_from_slice(bytes))
    }
}

impl From<Vec<u8>> for ContentData {
    fn from(vec: Vec<u8>) -> Self {
        Self::new(ContentSource::new(), Bytes::from(vec))
    }
}

impl From<Bytes> for ContentData {
    fn from(bytes: Bytes) -> Self {
        Self::new(ContentSource::new(), bytes)
    }
}

impl From<HipStr<'static>> for ContentData {
    fn from(text: HipStr<'static>) -> Self {
        Self::from_text(ContentSource::new(), text.to_string())
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
    fn sha256_computation() {
        let content = ContentData::from("Hello, world!");
        let hash = content.sha256();
        assert_eq!(hash.len(), 32);
        assert_eq!(hash, content.sha256());
    }

    #[test]
    fn sha256_verification() {
        let content = ContentData::from("Hello, world!");
        let hash = content.sha256().clone();
        assert!(content.verify_sha256(&hash).is_ok());
        assert!(content.verify_sha256([0u8; 32]).is_err());
    }

    #[test]
    fn as_hipstr() {
        let content = ContentData::from("Hello, HipStr!");
        assert_eq!(content.as_hipstr().unwrap().as_str(), "Hello, HipStr!");

        let binary = ContentData::from(vec![0xFF, 0xFE]);
        assert!(binary.as_hipstr().is_err());
    }

    #[test]
    fn slice_operations() {
        let content = ContentData::from("Hello, world!");
        assert_eq!(content.slice(0, 5).unwrap(), Bytes::from("Hello"));
        assert_eq!(content.slice(7, 12).unwrap(), Bytes::from("world"));
        assert!(content.slice(0, 100).is_err());
        assert!(content.slice(10, 5).is_err());
    }

    #[test]
    fn detect_mime_png() {
        let png = vec![
            0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48,
            0x44, 0x52,
        ];
        let data = ContentData::from(png);
        assert_eq!(data.detect_mime().as_deref(), Some("image/png"));
    }

    #[test]
    fn detect_mime_unknown() {
        let data = ContentData::from("hello world");
        assert_eq!(data.detect_mime(), None);
    }

    #[test]
    fn is_likely_text() {
        assert!(ContentData::from("ascii text").is_likely_text());
        assert!(ContentData::from("").is_likely_text());
        assert!(ContentData::from("café").is_likely_text());
        assert!(!ContentData::from(vec![0x00]).is_likely_text());
    }
}
