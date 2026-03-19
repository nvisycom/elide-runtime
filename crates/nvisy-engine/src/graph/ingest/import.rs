//! Import node configuration.
//!
//! [`ImportFile`] runs at **phase 0**, alongside [`LoadContext`]. It is always
//! the first processing step: it resolves previously uploaded content by UUID,
//! optionally decompresses and decrypts it, then feeds the raw bytes into the
//! pipeline envelope for downstream extraction.
//!
//! [`LoadContext`]: crate::graph::LoadContext

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

use super::{CompressionFormat, EncryptionFormat};

/// Configuration for the [`ImportFile`] graph node.
///
/// Identifies the content objects to load and specifies any decoding steps
/// that must be applied before the bytes are passed to extraction nodes.
///
/// [`ImportFile`]: crate::graph::GraphNodeKind::ImportFile
#[derive(Debug, Clone, Default, PartialEq, Eq, Validate)]
#[derive(Serialize, Deserialize, JsonSchema)]
pub struct ImportFile {
    /// Identifiers of previously uploaded content to import. Must contain at least one.
    #[validate(length(min = 1, message = "import requires at least one content_id"))]
    #[serde(default)]
    pub content_ids: Vec<Uuid>,
    /// Decompress the content before decoding.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decompression: Option<CompressionFormat>,
    /// Decrypt the content before decoding.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decryption: Option<EncryptionFormat>,
}
