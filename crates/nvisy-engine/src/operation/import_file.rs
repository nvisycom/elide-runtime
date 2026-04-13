//! File import operation.
//!
//! Runs at **phase 0**. Decodes raw content into a
//! [`DocumentEnvelope`], optionally applying decompression and
//! decryption.
//!
//! The import pipeline applies optional pre-processing steps in order:
//!
//! 1. **Decompression** — decompress raw bytes (if format specified)
//! 2. **Decryption** — decrypt content (if encryption config specified)
//! 3. **Decode** — detect format and decode into a typed [`ContentHandle`]
//!
//! [`ContentHandle`]: nvisy_codec::ContentHandle

use std::sync::Arc;

use nvisy_codec::ContentHandle;
use nvisy_core::Result;
use nvisy_core::content::{Content, ContentData};
use nvisy_ontology::workflow::{CompressionAlgorithm, EncryptionAlgorithm, EncryptionConfig};

use crate::operation::DocumentEnvelope;
use crate::operation::envelope::SharedData;
use crate::utility::compression::CompressionService;
use crate::utility::encryption::{CryptoService, EncryptedContent};

const TARGET: &str = "nvisy_engine::op::import_file";

/// Decodes raw content into a [`DocumentEnvelope`], optionally applying
/// decompression and decryption beforehand.
///
/// Not an [`Operation`] — import *creates* envelopes rather than
/// mutating them.
///
/// [`Operation`]: crate::operation::Operation
#[derive(Default)]
pub struct ImportFileOp {
    decompression: Option<CompressionAlgorithm>,
    decryption: Option<EncryptionConfig>,
}

impl ImportFileOp {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_decompression(mut self, format: Option<CompressionAlgorithm>) -> Self {
        self.decompression = format;
        self
    }

    pub fn with_decryption(mut self, config: Option<EncryptionConfig>) -> Self {
        self.decryption = config;
        self
    }

    pub async fn import(
        &self,
        content: Content,
        shared: &Arc<SharedData>,
    ) -> Result<DocumentEnvelope> {
        let mut content = content;

        if let Some(algorithm) = self.decompression {
            tracing::debug!(target: TARGET, ?algorithm, "decompressing content");
            let decompressed = CompressionService::new(algorithm).decompress(content.as_bytes())?;
            let source = content.content_source();
            content = replace_data(content, ContentData::new(source, decompressed));
        }

        if let Some(ref enc_cfg) = self.decryption {
            tracing::debug!(target: TARGET, key_id = %enc_cfg.key_id, "decrypting content");
            let crypto = CryptoService::new(&enc_cfg.key_id, shared.key_provider.clone());
            let encrypted = EncryptedContent {
                source: content.content_source(),
                ciphertext: bytes::Bytes::copy_from_slice(content.as_bytes()),
                key_id: enc_cfg.key_id.clone(),
                algorithm: EncryptionAlgorithm::Aes256Gcm,
            };
            let decrypted_data = crypto.decrypt(encrypted).await?;
            content = replace_data(content, decrypted_data);
        }

        let doc = ContentHandle::decode(&content).await?;
        tracing::debug!(target: TARGET, doc_type = %doc.document_type(), "decoded document");
        let mut metadata = content.into_parts().1.unwrap_or_default();

        // Move persisted annotations from metadata to the envelope
        // and apply inclusions as entities.
        let annotations = std::mem::take(&mut metadata.annotations);
        let mut envelope = DocumentEnvelope::new(doc, metadata, Arc::clone(shared));
        if !annotations.is_empty() {
            annotations.apply_inclusions(&mut envelope.audit.entities);
            envelope.annotations = annotations;
        }
        Ok(envelope)
    }
}

/// Replace the data payload of a [`Content`] while preserving its metadata.
fn replace_data(content: Content, data: ContentData) -> Content {
    match content.into_parts().1 {
        Some(meta) => Content::with_metadata(data, meta),
        None => Content::new(data),
    }
}

#[cfg(test)]
mod tests {
    use nvisy_core::content::{Content, ContentData};

    use super::*;
    use crate::operation::envelope::SharedData;

    #[tokio::test]
    async fn unknown_format_errors() {
        let dir = tempfile::tempdir().unwrap();
        let registry = crate::registry::Registry::open(dir.path()).unwrap();
        let shared = SharedData::new(uuid::Uuid::new_v4(), uuid::Uuid::new_v4(), registry);
        let content = Content::new(ContentData::from("plain text has no magic bytes"));
        assert!(ImportFileOp::new().import(content, &shared).await.is_err());
    }
}
