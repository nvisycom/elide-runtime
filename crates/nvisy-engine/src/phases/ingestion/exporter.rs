//! File export operation.
//!
//! Delivers processed content to the registry, optionally applying
//! encryption and compression in order:
//!
//! 1. **Encryption** — encrypt content (if config specified)
//! 2. **Compression** — compress for storage or transfer (if format specified)

use std::sync::Arc;

use nvisy_codec::content::{Content, ContentData, ContentSource};
use nvisy_codec::core::Codable;
use nvisy_core::Result;
use uuid::Uuid;

use crate::core::{AnyTree, DocumentTree, SharedData};
use crate::modality::DocumentModality;
use crate::phases::ingestion::compression::CompressionService;
use crate::phases::ingestion::encryption::CryptoService;
use crate::phases::ingestion::{CompressionAlgorithm, EncryptionConfig};

const TARGET: &str = "nvisy_engine::op::export_file";

/// Exports processed content, optionally applying encryption and
/// compression afterward.
#[derive(Default)]
pub(crate) struct Exporter {
    encryption: Option<EncryptionConfig>,
    compression: Option<CompressionAlgorithm>,
    content_ids: Vec<Uuid>,
}

impl Exporter {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn with_encryption(mut self, config: Option<EncryptionConfig>) -> Self {
        self.encryption = config;
        self
    }

    pub(crate) fn with_compression(mut self, format: Option<CompressionAlgorithm>) -> Self {
        self.compression = format;
        self
    }

    pub(crate) fn with_content_ids(mut self, ids: Vec<Uuid>) -> Self {
        self.content_ids = ids;
        self
    }

    /// Encode the tree's codec handle back to bytes, optionally
    /// encrypt + compress, and write to the registry under every
    /// configured content id.
    pub(crate) async fn export(&self, tree: &AnyTree, shared: &Arc<SharedData>) -> Result<()> {
        match tree {
            AnyTree::Text(t) => self.export_one(t, shared).await,
            AnyTree::Tabular(t) => self.export_one(t, shared).await,
            AnyTree::Image(t) => self.export_one(t, shared).await,
            AnyTree::Audio(t) => self.export_one(t, shared).await,
        }
    }

    async fn export_one<M>(&self, tree: &DocumentTree<M>, shared: &Arc<SharedData>) -> Result<()>
    where
        M: DocumentModality + Codable,
    {
        let content_data = tree.handle.handler().encode()?;
        let mut output_bytes = bytes::Bytes::copy_from_slice(content_data.as_bytes());

        if let Some(ref enc_cfg) = self.encryption {
            tracing::debug!(target: TARGET, key_id = %enc_cfg.key_id, "encrypting export content");
            let crypto = CryptoService::new(&enc_cfg.key_id, shared.key_provider.clone());
            let encrypted = crypto.encrypt(&content_data).await?;
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
            let source = ContentSource::from_uuid_unchecked(content_id);
            let data = ContentData::new(source, output_bytes.clone());
            let content = Content::new(data);
            shared
                .registry
                .register_content(shared.actor_id, content, None)
                .await?;
            tracing::debug!(target: TARGET, %content_id, "wrote exported content to registry");
        }

        tracing::debug!(target: TARGET, "export complete");
        Ok(())
    }
}
