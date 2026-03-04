//! Base64-encoded string wrapper.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// A base64-encoded string.
///
/// Wraps a [`String`] expected to contain standard base64-encoded data.
/// Provides typed documentation in OpenAPI schemas and enforces
/// the encoding contract at the type level.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(transparent)]
pub struct Base64(String);

impl Base64 {
    /// Returns the raw base64 string.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Decodes the base64 content into raw bytes.
    ///
    /// # Errors
    ///
    /// Returns a [`base64::DecodeError`] if the string is not valid base64.
    pub fn decode(&self) -> std::result::Result<Vec<u8>, base64::DecodeError> {
        use base64::Engine as _;
        base64::engine::general_purpose::STANDARD.decode(&self.0)
    }
}

impl From<String> for Base64 {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<Base64> for String {
    fn from(b: Base64) -> Self {
        b.0
    }
}
