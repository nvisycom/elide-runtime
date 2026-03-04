use std::ops::Deref;

use bytes::Bytes;
use derive_more::{AsRef, From};
use hipstr::HipStr;
use serde::{Deserialize, Serialize};

use crate::error::{Error, ErrorKind, Result};

/// A wrapper around `Bytes` for content storage.
///
/// This struct wraps `bytes::Bytes` and provides additional methods
/// for text conversion. It's cheap to clone as `Bytes` uses reference
/// counting internally.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[derive(From, AsRef, Serialize, Deserialize)]
#[as_ref(forward)]
#[serde(transparent)]
pub struct ContentBytes(Bytes);

impl ContentBytes {
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
        let s = std::str::from_utf8(&self.0)
            .map_err(|e| Error::new(ErrorKind::Serialization, format!("Invalid UTF-8: {e}")))?;
        Ok(HipStr::from(s))
    }

    /// Consumes the wrapper, returning the inner `Bytes`.
    #[must_use]
    pub fn into_inner(self) -> Bytes {
        self.0
    }

    /// Returns `true` if the content appears to be text.
    ///
    /// Checks that the content is valid UTF-8 and contains no control
    /// characters other than common whitespace (tab, newline, carriage
    /// return). This covers ASCII text as well as multi-byte scripts
    /// (e.g. CJK, Cyrillic, emoji).
    #[must_use]
    pub fn is_likely_text(&self) -> bool {
        let Ok(s) = std::str::from_utf8(&self.0) else {
            return false;
        };
        s.chars()
            .all(|c| !c.is_control() || matches!(c, '\t' | '\n' | '\r'))
    }
}

impl Deref for ContentBytes {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_len_and_is_empty() {
        let empty = ContentBytes::default();
        assert_eq!(empty.len(), 0);
        assert!(empty.is_empty());

        let nonempty = ContentBytes::from("hello");
        assert_eq!(nonempty.len(), 5);
        assert!(!nonempty.is_empty());
    }

    #[test]
    fn test_as_bytes() {
        let cb = ContentBytes::from("abc");
        assert_eq!(cb.as_bytes(), b"abc");
    }

    #[test]
    fn test_as_str() {
        let text = ContentBytes::from("valid utf-8");
        assert_eq!(text.as_str(), Some("valid utf-8"));

        let binary = ContentBytes::from(vec![0xFF, 0xFE]);
        assert_eq!(binary.as_str(), None);
    }

    #[test]
    fn test_as_hipstr() {
        let text = ContentBytes::from("hipstr");
        assert_eq!(text.as_hipstr().unwrap().as_str(), "hipstr");

        let binary = ContentBytes::from(vec![0xFF]);
        assert!(binary.as_hipstr().is_err());
    }

    #[test]
    fn test_into_inner() {
        let cb = ContentBytes::from("inner");
        let bytes = cb.into_inner();
        assert_eq!(bytes, Bytes::from("inner"));
    }

    #[test]
    fn test_is_likely_text() {
        assert!(ContentBytes::from("ascii text").is_likely_text());
        assert!(ContentBytes::from("").is_likely_text());
        assert!(ContentBytes::from("café").is_likely_text());

        assert!(!ContentBytes::from(vec![0x00]).is_likely_text());
        assert!(!ContentBytes::from(vec![0x89, 0x50, 0x4E, 0x47]).is_likely_text()); // PNG header
    }

    #[test]
    fn test_deref_to_slice() {
        let cb = ContentBytes::from("deref");
        let slice: &[u8] = &cb;
        assert_eq!(slice, b"deref");
    }

    #[test]
    fn test_from_conversions() {
        let from_str = ContentBytes::from("test");
        let from_string = ContentBytes::from("test".to_string());
        let from_bytes = ContentBytes::from(b"test".as_slice());
        let from_vec = ContentBytes::from(b"test".to_vec());
        let from_hipstr = ContentBytes::from(HipStr::from("test"));

        assert_eq!(from_str.as_bytes(), b"test");
        assert_eq!(from_string.as_bytes(), b"test");
        assert_eq!(from_bytes.as_bytes(), b"test");
        assert_eq!(from_vec.as_bytes(), b"test");
        assert_eq!(from_hipstr.as_bytes(), b"test");
    }
}
