//! Lifecycle action configurations: import and export.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Configuration for the [`Import`](super::GraphNodeKind::Import) action.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[derive(Serialize, Deserialize, JsonSchema)]
pub struct Import {
    /// Decompress the content before processing.
    #[serde(default)]
    pub decompression: bool,
    /// Decrypt the content before processing.
    #[serde(default)]
    pub decryption: bool,
    /// Convert the content to a processable format.
    #[serde(default)]
    pub conversion: bool,
}

/// Configuration for the [`Export`](super::GraphNodeKind::Export) action.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[derive(Serialize, Deserialize, JsonSchema)]
pub struct Export {
    /// Compress the content before publishing.
    #[serde(default)]
    pub compression: bool,
    /// Encrypt the content before publishing.
    #[serde(default)]
    pub encryption: bool,
    /// Convert the content to the target format.
    #[serde(default)]
    pub conversion: bool,
}
