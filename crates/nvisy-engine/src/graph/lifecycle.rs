//! Lifecycle action configurations: import and export.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Supported compression formats for import/export.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CompressionFormat {
    /// Gzip (.gz).
    Gzip,
    /// Zstandard (.zst).
    Zstd,
}

/// Supported encryption formats for import/export.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum EncryptionFormat {
    /// AES-256 in Galois/Counter Mode.
    Aes256Gcm,
}

/// Configuration for the [`Import`] action.
///
/// [`Import`]: super::GraphNodeKind::Import
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[derive(Serialize, Deserialize, JsonSchema)]
pub struct Import {
    /// Identifiers of previously uploaded content to import.
    #[serde(default)]
    pub content_ids: Vec<Uuid>,
    /// Decompress the content before decoding.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decompression: Option<CompressionFormat>,
    /// Decrypt the content before decoding.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decryption: Option<EncryptionFormat>,
}

/// Configuration for the [`Export`] action.
///
/// [`Export`]: super::GraphNodeKind::Export
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[derive(Serialize, Deserialize, JsonSchema)]
pub struct Export {
    /// Identifiers of content destinations to export to.
    #[serde(default)]
    pub content_ids: Vec<Uuid>,
    /// Encrypt the content before publishing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub encryption: Option<EncryptionFormat>,
    /// Compress the content before publishing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compression: Option<CompressionFormat>,
}
