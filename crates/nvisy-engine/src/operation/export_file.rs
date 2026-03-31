//! File export operation.
//!
//! Runs at **phase 6** alongside [`SaveContext`]. Delivers processed
//! content, optionally applying encryption and compression.
//!
//! [`SaveContext`]: crate::operation::SaveContextOp

//! The export pipeline applies optional post-processing steps in order:
//!
//! 1. **Encryption** — encrypt content (if config specified)
//! 2. **Compression** — compress for storage or transfer (if format specified)

use nvisy_core::Result;
use nvisy_ontology::workflow::{CompressionAlgorithm, EncryptionConfig};
use uuid::Uuid;

use crate::operation::DocumentEnvelope;
use crate::operation::compression::CompressionService;
use crate::operation::encryption::CryptoService;

const TARGET: &str = "nvisy_engine::op::export_file";

/// Exports processed content, optionally applying encryption and
/// compression afterward.
///
/// Not an [`Operation`] — export *consumes* envelopes to produce
/// registry artifacts rather than mutating them in place.
///
/// [`Operation`]: crate::operation::Operation
#[derive(Default)]
pub struct ExportFileOp {
    encryption: Option<EncryptionConfig>,
    compression: Option<CompressionAlgorithm>,
    content_ids: Vec<Uuid>,
}

impl ExportFileOp {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_encryption(mut self, config: Option<EncryptionConfig>) -> Self {
        self.encryption = config;
        self
    }

    pub fn with_compression(mut self, format: Option<CompressionAlgorithm>) -> Self {
        self.compression = format;
        self
    }

    pub fn with_content_ids(mut self, ids: Vec<Uuid>) -> Self {
        self.content_ids = ids;
        self
    }

    pub async fn export(&self, envelope: &DocumentEnvelope) -> Result<()> {
        let shared = &envelope.shared;
        let content_data = envelope.document.encode()?;
        let mut output_bytes = bytes::Bytes::copy_from_slice(content_data.as_bytes());

        if let Some(ref enc_cfg) = self.encryption {
            tracing::debug!(target: TARGET, key_id = %enc_cfg.key_id, "encrypting export content");
            let crypto = CryptoService::new(&enc_cfg.key_id, shared.key_provider.clone());
            let encrypted = crypto.encrypt(envelope).await?;
            tracing::debug!(
                target: TARGET,
                ciphertext_len = encrypted.ciphertext.len(),
                "content encrypted",
            );
            output_bytes = encrypted.ciphertext;
        }

        if let Some(algorithm) = self.compression {
            tracing::debug!(target: TARGET, ?algorithm, "compressing export content");
            output_bytes = CompressionService::new(algorithm).compress(&output_bytes)?;
        }

        for &content_id in &self.content_ids {
            use nvisy_core::content::{Content, ContentData, ContentSource};
            let source = ContentSource::from_uuid(content_id);
            let data = ContentData::new(source, output_bytes.clone());
            let content = Content::new(data);
            shared
                .registry
                .register_content(shared.actor_id, content)
                .await?;
            tracing::debug!(target: TARGET, %content_id, "wrote exported content to registry");
        }

        tracing::debug!(target: TARGET, "export complete");
        Ok(())
    }
}
