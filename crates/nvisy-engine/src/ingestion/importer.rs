//! File import operation.
//!
//! Decodes raw content into a [`DocumentEnvelope<Text>`], optionally
//! applying pre-processing in order:
//!
//! 1. **Decompression** — decompress raw bytes (if format specified)
//! 2. **Decryption** — decrypt content (if encryption config specified)
//! 3. **Decode** — detect format and decode into a typed [`DocumentHandle`]
//!
//! [`DocumentHandle`]: nvisy_codec::DocumentHandle

use nvisy_ontology::modality::Text;
use std::mem;
use std::sync::Arc;

use nvisy_core::Result;
use nvisy_core::content::{Content, ContentData};

use crate::envelope::{DocumentEnvelope, SharedData};
use crate::ingestion::compression::CompressionService;
use crate::ingestion::encryption::{CryptoService, EncryptedContent};
use crate::ingestion::{CompressionAlgorithm, EncryptionAlgorithm, EncryptionConfig};

const TARGET: &str = "nvisy_engine::op::import_file";

/// Decodes raw content into a [`DocumentEnvelope<Text>`], optionally applying
/// decompression and decryption beforehand.
#[derive(Default)]
pub struct Importer {
    decompression: Option<CompressionAlgorithm>,
    decryption: Option<EncryptionConfig>,
}

impl Importer {
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
    ) -> Result<DocumentEnvelope<Text>> {
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

        let doc = nvisy_formats::decode(&content).await?;
        tracing::debug!(target: TARGET, doc_type = %doc.document_type(), "decoded document");
        let mut metadata = content.into_parts().1.unwrap_or_default();

        // Move persisted annotations from metadata to the envelope.
        // Inclusion entity seeding is reinstated once annotations are
        // typed per modality.
        let _annotations = mem::take(&mut metadata.annotations);
        let envelope = <DocumentEnvelope<Text>>::new(
            std::sync::Arc::new(tokio::sync::Mutex::new(doc)),
            metadata,
            Arc::clone(shared),
        )
        .await;
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
    use crate::envelope::SharedData;

    #[tokio::test]
    async fn unknown_format_errors() {
        let dir = tempfile::tempdir().unwrap();
        let registry = crate::ingestion::registry::Registry::open(dir.path()).unwrap();
        let shared = SharedData::new(uuid::Uuid::new_v4(), uuid::Uuid::new_v4(), registry);
        let content = Content::new(ContentData::from("plain text has no magic bytes"));
        assert!(Importer::new().import(content, &shared).await.is_err());
    }
}
