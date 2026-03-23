//! File import operation.
//!
//!
//! Runs at **phase 0** alongside [`LoadContext`]. Decodes raw content
//! into a [`DocumentEnvelope`], optionally applying decompression and
//! decryption.
//!
//! [`LoadContext`]: crate::operation::LoadContext

//! The import pipeline applies optional pre-processing steps in order:
//!
//! 1. **Decompression** — decompress raw bytes (if format specified)
//! 2. **Decryption** — decrypt content (if encryption config specified)
//! 3. **Decode** — detect format and decode into a typed [`Document`]
//!
//! [`Document`]: nvisy_codec::Document

use nvisy_codec::Document;
use nvisy_core::Result;
use nvisy_core::content::ContentData;

use crate::graph::{CompressionAlgorithm, EncryptionAlgorithm, EncryptionConfig};
use crate::operation::compression::CompressionService;
use crate::operation::context::{ParallelContext, SharedContext};
use crate::operation::encryption::{CryptoService, EncryptedContent};
use crate::operation::{DocumentEnvelope, Operation};

const TARGET: &str = "nvisy_engine::op::import_file";

/// Decodes raw content into a [`DocumentEnvelope`], optionally applying
/// decompression and decryption beforehand.
///
/// Key provider is read from the [`SharedContext`] at call time —
/// only graph-config fields are stored on the struct.
#[derive(Default)]
pub struct ImportFile {
    decompression: Option<CompressionAlgorithm>,
    decryption: Option<EncryptionConfig>,
}

impl ImportFile {
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

    async fn import(
        &self,
        content: ContentData,
        shared: &SharedContext,
    ) -> Result<DocumentEnvelope> {
        let mut data = content;

        if let Some(algorithm) = self.decompression {
            tracing::debug!(target: TARGET, ?algorithm, "decompressing content");
            let decompressed = CompressionService::new(algorithm).decompress(data.as_bytes())?;
            let mut new_data = ContentData::new(data.content_source, decompressed);
            new_data.filename = data.filename;
            new_data.supplied_mime = data.supplied_mime;
            data = new_data;
        }

        if let Some(ref enc_cfg) = self.decryption {
            tracing::debug!(target: TARGET, key_id = %enc_cfg.key_id, "decrypting content");
            let crypto = CryptoService::new(&enc_cfg.key_id, shared.key_provider.clone());
            let encrypted = EncryptedContent {
                source: data.content_source,
                ciphertext: bytes::Bytes::copy_from_slice(data.as_bytes()),
                key_id: enc_cfg.key_id.clone(),
                algorithm: EncryptionAlgorithm::Aes256Gcm,
                filename: data.filename.clone(),
            };
            data = crypto.decrypt(encrypted).await?;
        }

        let doc = Document::decode(&data).await?;
        tracing::debug!(target: TARGET, doc_type = %doc.document_type(), "decoded document");
        Ok(DocumentEnvelope::new(doc))
    }
}

impl Operation for ImportFile {
    type Input = ParallelContext<ContentData>;
    type Output = ParallelContext<DocumentEnvelope>;

    async fn call(&self, input: Self::Input) -> Result<Self::Output> {
        let shared = input.shared.clone();
        input.parallel_map(|data| self.import(data, &shared)).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::operation::context::SharedContext;

    #[tokio::test]
    async fn unknown_format_errors() {
        let dir = tempfile::tempdir().unwrap();
        let registry = crate::registry::Registry::open(dir.path()).unwrap();
        let shared = SharedContext::new(uuid::Uuid::new_v4(), uuid::Uuid::new_v4(), registry);
        let content = ContentData::from("plain text has no magic bytes");
        let input = ParallelContext::new(content, shared);
        assert!(ImportFile::new().call(input).await.is_err());
    }
}
