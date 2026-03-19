//! Import node configuration.
//!
//! [`ImportFile`] runs at **phase 0**, alongside [`LoadContext`]. It is always
//! the first processing step: it resolves previously uploaded content by UUID,
//! optionally decompresses and decrypts it, then feeds the raw bytes into the
//! pipeline envelope for downstream extraction.
//!
//! [`LoadContext`]: crate::graph::LoadContext

use nvisy_codec::Document;
use nvisy_core::{Error, Result};
use nvisy_registry::Registry;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

use super::{CompressionFormat, EncryptionFormat};
use crate::operation::DocumentEnvelope;

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

impl ImportFile {
    /// Load all content from the registry, decode each into a
    /// [`DocumentEnvelope`].
    pub async fn load(&self, registry: &Registry, actor_id: Uuid) -> Result<Vec<DocumentEnvelope>> {
        if self.decompression.is_some() {
            return Err(Error::runtime(
                format!(
                    "import decompression ({:?}) is not yet implemented",
                    self.decompression
                ),
                "import",
                false,
            ));
        }
        if self.decryption.is_some() {
            return Err(Error::runtime(
                format!(
                    "import decryption ({:?}) is not yet implemented",
                    self.decryption
                ),
                "import",
                false,
            ));
        }

        let mut envelopes = Vec::with_capacity(self.content_ids.len());
        for &content_id in &self.content_ids {
            let handle = registry.read_content(actor_id, content_id).await?;
            let content_data = handle.content_data().await?;
            let doc = Document::decode(&content_data).await?;
            envelopes.push(DocumentEnvelope::new(doc));
        }
        Ok(envelopes)
    }
}
