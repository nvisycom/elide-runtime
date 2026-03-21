//! File export operation.
//!
//!
//! Runs at **phase 6** alongside [`SaveContext`]. Delivers processed
//! content, optionally applying encryption and compression.
//!
//! [`SaveContext`]: crate::operation::SaveContext

//! The export pipeline applies optional post-processing steps in order:
//!
//! 1. **Encryption** — encrypt content (if config specified)
//! 2. **Compression** — compress for storage or transfer (if format specified)

use std::sync::Arc;

use nvisy_core::Result;
use nvisy_registry::Registry;
use uuid::Uuid;

use crate::graph::{CompressionAlgorithm, EncryptionConfig};
use crate::operation::compression::CompressionService;
use crate::operation::context::ParallelContext;
use crate::operation::encryption::{CryptoService, KeyProvider};
use crate::operation::{DocumentEnvelope, Operation};

const TARGET: &str = "nvisy_engine::op::export_file";

/// Exports processed content, optionally applying encryption and
/// compression afterward.
pub struct ExportFile {
    encryption: Option<EncryptionConfig>,
    compression: Option<CompressionAlgorithm>,
    key_provider: Option<Arc<dyn KeyProvider>>,
    registry: Option<Registry>,
    actor_id: Uuid,
    content_ids: Vec<Uuid>,
}

impl ExportFile {
    pub fn new() -> Self {
        Self {
            encryption: None,
            compression: None,
            key_provider: None,
            registry: None,
            actor_id: Uuid::nil(),
            content_ids: Vec::new(),
        }
    }

    pub fn with_encryption(mut self, config: Option<EncryptionConfig>) -> Self {
        self.encryption = config;
        self
    }

    pub fn with_compression(mut self, format: Option<CompressionAlgorithm>) -> Self {
        self.compression = format;
        self
    }

    pub fn with_key_provider(mut self, provider: Arc<dyn KeyProvider>) -> Self {
        self.key_provider = Some(provider);
        self
    }

    pub fn with_registry(mut self, registry: Registry, actor_id: Uuid) -> Self {
        self.registry = Some(registry);
        self.actor_id = actor_id;
        self
    }

    pub fn with_content_ids(mut self, ids: Vec<Uuid>) -> Self {
        self.content_ids = ids;
        self
    }

    async fn export(&self, envelope: DocumentEnvelope) -> Result<DocumentEnvelope> {
        let content_data = envelope.document.encode()?;
        let mut output_bytes = bytes::Bytes::copy_from_slice(content_data.as_bytes());

        if let Some(ref enc_cfg) = self.encryption {
            let key_provider = self.key_provider.as_ref().ok_or_else(|| {
                nvisy_core::Error::runtime(
                    "encryption requires a KeyProvider",
                    "export_file",
                    false,
                )
            })?;
            tracing::debug!(target: TARGET, key_id = %enc_cfg.key_id, "encrypting export content");
            let crypto = CryptoService::new(&enc_cfg.key_id, Arc::clone(key_provider));
            let encrypted = crypto.encrypt(&envelope).await?;
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

        if let Some(ref registry) = self.registry {
            for &content_id in &self.content_ids {
                use nvisy_core::content::{Content, ContentData, ContentSource};
                let source = ContentSource::from_uuid(content_id);
                let data = ContentData::new(source, output_bytes.clone());
                let content = Content::new(data);
                registry.register_content(self.actor_id, content).await?;
                tracing::debug!(target: TARGET, %content_id, "wrote exported content to registry");
            }
        }

        tracing::debug!(target: TARGET, "export complete");
        Ok(envelope)
    }
}

impl Default for ExportFile {
    fn default() -> Self {
        Self::new()
    }
}

impl Operation for ExportFile {
    type Input = ParallelContext<DocumentEnvelope>;
    type Output = ParallelContext<DocumentEnvelope>;

    async fn call(&self, input: Self::Input) -> Result<Self::Output> {
        input.parallel_map(|data| self.export(data)).await
    }
}
