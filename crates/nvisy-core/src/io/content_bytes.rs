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
