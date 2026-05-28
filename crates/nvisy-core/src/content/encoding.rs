//! Character encoding for text-based loaders.

use crate::error::{Error, Result};

/// Character encoding used to decode raw bytes before parsing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TextEncoding {
    /// UTF-8 (the default and by far the most common encoding).
    #[default]
    Utf8,
}

impl TextEncoding {
    /// Decode raw bytes to a UTF-8 string.
    ///
    /// `origin` identifies the caller for error messages
    /// (e.g. `"json-loader"`).
    pub fn decode_bytes(self, bytes: &[u8], origin: &str) -> Result<String> {
        match self {
            Self::Utf8 => String::from_utf8(bytes.to_vec())
                .map_err(|e| Error::validation(format!("Invalid UTF-8: {e}"), origin.to_owned())),
        }
    }
}
