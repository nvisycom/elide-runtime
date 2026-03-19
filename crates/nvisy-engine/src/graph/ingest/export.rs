//! Export node configuration.
//!
//! [`ExportFile`] runs at **phase 6**, alongside [`SaveContext`]. It delivers
//! the processed (and optionally redacted) content to one or more destination
//! content objects, applying encryption and compression as requested before
//! writing the bytes out.
//!
//! [`SaveContext`]: crate::graph::SaveContext

use nvisy_core::content::Content;
use nvisy_core::{Error, Result};
use nvisy_registry::Registry;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{CompressionFormat, EncryptionFormat};
use crate::operation::DocumentEnvelope;

/// Configuration for the [`ExportFile`] graph node.
///
/// Identifies the destination content objects and specifies any encoding
/// steps that must be applied before the bytes are written out.
///
/// [`ExportFile`]: crate::graph::GraphNodeKind::ExportFile
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[derive(Serialize, Deserialize, JsonSchema)]
pub struct ExportFile {
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

impl ExportFile {
    /// Encode each envelope's document and write to the registry.
    /// Returns the number of content objects saved.
    pub async fn save(
        &self,
        registry: &Registry,
        actor_id: Uuid,
        envelopes: &[DocumentEnvelope],
    ) -> Result<usize> {
        if self.encryption.is_some() {
            return Err(Error::runtime(
                format!(
                    "export encryption ({:?}) is not yet implemented",
                    self.encryption
                ),
                "export",
                false,
            ));
        }
        if self.compression.is_some() {
            return Err(Error::runtime(
                format!(
                    "export compression ({:?}) is not yet implemented",
                    self.compression
                ),
                "export",
                false,
            ));
        }

        let mut saved = 0usize;
        for envelope in envelopes {
            let content_data = envelope.document.encode()?;
            let content = Content::new(content_data);
            registry.register_content(actor_id, content).await?;
            saved += 1;
        }
        Ok(saved)
    }
}
