//! Lifecycle action configurations: import and export.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Configuration for the [`Import`] action.
///
/// [`Import`]: super::GraphNodeKind::Import
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[derive(Serialize, Deserialize, JsonSchema)]
pub struct Import {
    /// Identifiers of previously uploaded content to import.
    #[serde(default)]
    pub content_ids: Vec<Uuid>,
    /// Decompress the content before processing.
    #[serde(default)]
    pub decompression: bool,
    /// Decrypt the content before processing.
    #[serde(default)]
    pub decryption: bool,
}

/// Configuration for the [`Export`] action.
///
/// [`Export`]: super::GraphNodeKind::Export
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[derive(Serialize, Deserialize, JsonSchema)]
pub struct Export {
    /// Compress the content before publishing.
    #[serde(default)]
    pub compression: bool,
    /// Encrypt the content before publishing.
    #[serde(default)]
    pub encryption: bool,
}
