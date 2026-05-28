//! AES-256-GCM encryption and decryption service.

use aes_gcm::aead::Aead;
use aes_gcm::{Aes256Gcm, KeyInit, Nonce};
use bytes::Bytes;
use nvisy_core::content::ContentData;
use nvisy_core::{Error, ErrorKind, Result};
use nvisy_ontology::modality::Modality;
use rand::RngExt;

use super::provider::{KeyProvider, SharedKeyProvider};
use super::wire::{EncryptedContent, NONCE_SIZE, WireEnvelope};
use crate::envelope::DocumentEnvelope;
use crate::ingestion::EncryptionAlgorithm;

const TARGET: &str = "nvisy_engine::op::encryption";

/// AES-256-GCM encryption and decryption service.
pub struct CryptoService {
    key_id: String,
    key_provider: SharedKeyProvider,
}

impl CryptoService {
    /// Creates a new crypto service.
    pub fn new(key_id: impl Into<String>, key_provider: SharedKeyProvider) -> Self {
        Self {
            key_id: key_id.into(),
            key_provider,
        }
    }

    /// Encrypt a [`DocumentEnvelope<M>`] into an [`EncryptedContent`] blob.
    /// Modality-agnostic — uses only the envelope's encoded bytes
    /// and content source.
    pub async fn encrypt<M: Modality>(
        &self,
        envelope: &DocumentEnvelope<M>,
    ) -> Result<EncryptedContent> {
        let content_data = envelope.encode().await?;
        let source = content_data.content_source;
        let plaintext = content_data.as_bytes();

        let raw_key = self.key_provider.resolve(&self.key_id)?;
        let cipher = Aes256Gcm::new_from_slice(&raw_key).map_err(|e| {
            Error::validation(
                format!("invalid AES-256 key: {e}"),
                "CryptoService::encrypt",
            )
        })?;

        let mut nonce_bytes = [0u8; NONCE_SIZE];
        rand::rng().fill(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        let ciphertext = cipher.encrypt(nonce, plaintext).map_err(|e| {
            Error::new(
                ErrorKind::Internal,
                format!("AES-256-GCM encryption failed: {e}"),
            )
        })?;

        let wire = WireEnvelope {
            algorithm: EncryptionAlgorithm::Aes256Gcm,
            key_id: &self.key_id,
            nonce: &nonce_bytes,
            ciphertext: &ciphertext,
        }
        .build();

        tracing::debug!(
            target: TARGET,
            key_id = %self.key_id,
            plaintext_len = plaintext.len(),
            wire_len = wire.len(),
            "encrypted content",
        );

        Ok(EncryptedContent {
            source,
            ciphertext: wire,
            key_id: self.key_id.clone(),
            algorithm: EncryptionAlgorithm::Aes256Gcm,
        })
    }

    /// Decrypt an [`EncryptedContent`] blob back to raw [`ContentData`].
    pub async fn decrypt(&self, encrypted: EncryptedContent) -> Result<ContentData> {
        let envelope = WireEnvelope::parse(&encrypted.ciphertext)?;

        if envelope.algorithm != EncryptionAlgorithm::Aes256Gcm {
            return Err(Error::validation(
                format!("unsupported algorithm: {:?}", envelope.algorithm),
                "CryptoService::decrypt",
            ));
        }

        if envelope.key_id != self.key_id {
            return Err(Error::validation(
                format!(
                    "key_id mismatch: wire envelope contains '{}' but service expects '{}'",
                    envelope.key_id, self.key_id,
                ),
                "CryptoService::decrypt",
            ));
        }

        tracing::debug!(
            target: TARGET,
            key_id = %encrypted.key_id,
            algorithm = ?encrypted.algorithm,
            "decrypting content",
        );

        let raw_key = self.key_provider.resolve(envelope.key_id)?;
        let cipher = Aes256Gcm::new_from_slice(&raw_key).map_err(|e| {
            Error::validation(
                format!("invalid AES-256 key: {e}"),
                "CryptoService::decrypt",
            )
        })?;

        let nonce = Nonce::from_slice(envelope.nonce);
        let plaintext = cipher.decrypt(nonce, envelope.ciphertext).map_err(|e| {
            Error::validation(
                format!("AES-256-GCM decryption failed (authentication error): {e}"),
                "CryptoService::decrypt",
            )
        })?;

        tracing::debug!(
            target: TARGET,
            key_id = %envelope.key_id,
            ciphertext_len = envelope.ciphertext.len(),
            plaintext_len = plaintext.len(),
            "decrypted content",
        );

        let content = ContentData::new(encrypted.source, Bytes::from(plaintext));
        Ok(content)
    }
}

#[cfg(test)]
mod tests {
    use nvisy_core::content::{Content, ContentData, ContentMetadata, ContentSource};
    use nvisy_ontology::modality::Text;

    use super::*;
    use crate::envelope::DocumentEnvelope;
    use crate::ingestion::encryption::{SharedKeyProvider, StaticKeyProvider};

    fn test_key_provider() -> SharedKeyProvider {
        let key = vec![0xAB; 32];
        SharedKeyProvider::new(StaticKeyProvider::new([("test-key".to_string(), key)]))
    }

    async fn test_envelope() -> DocumentEnvelope<Text> {
        let data = ContentData::from_text(ContentSource::new(), "Hello, world!");
        let meta = ContentMetadata::new().with_content_type("text/plain");
        let content = Content::with_metadata(data, meta);
        let doc = nvisy_formats::decode(&content).await.expect("decode text");
        let dir = tempfile::tempdir().unwrap();
        let registry = crate::ingestion::registry::Registry::open(dir.path()).unwrap();
        let shared =
            crate::envelope::SharedData::new(uuid::Uuid::new_v4(), uuid::Uuid::new_v4(), registry);
        <DocumentEnvelope<Text>>::new(
            std::sync::Arc::new(tokio::sync::Mutex::new(doc)),
            ContentMetadata::new().with_content_type("text/plain"),
            shared,
        )
        .await
    }

    #[tokio::test]
    async fn round_trip_encrypt_decrypt() {
        let provider = test_key_provider();
        let envelope = test_envelope().await;
        let original_bytes = envelope.encode().await.expect("encode").into_bytes();

        let svc = CryptoService::new("test-key", provider);
        let encrypted = svc.encrypt(&envelope).await.expect("encrypt");

        assert_eq!(encrypted.key_id, "test-key");
        assert_eq!(encrypted.algorithm, EncryptionAlgorithm::Aes256Gcm);
        assert_ne!(encrypted.ciphertext, original_bytes);

        let decrypted = svc.decrypt(encrypted).await.expect("decrypt");
        assert_eq!(decrypted.as_bytes(), &original_bytes[..]);
    }

    #[tokio::test]
    async fn wrong_key_fails_decryption() {
        let provider = test_key_provider();
        let envelope = test_envelope().await;

        let enc_svc = CryptoService::new("test-key", provider);
        let encrypted = enc_svc.encrypt(&envelope).await.expect("encrypt");

        let wrong_provider = SharedKeyProvider::new(StaticKeyProvider::new([(
            "test-key".to_string(),
            vec![0xCD; 32],
        )]));
        let dec_svc = CryptoService::new("test-key", wrong_provider);
        assert!(dec_svc.decrypt(encrypted).await.is_err());
    }

    #[tokio::test]
    async fn unknown_key_id_fails() {
        let empty_provider = SharedKeyProvider::new(StaticKeyProvider::new([]));
        let envelope = test_envelope().await;

        let svc = CryptoService::new("nonexistent", empty_provider);
        assert!(svc.encrypt(&envelope).await.is_err());
    }

    #[tokio::test]
    async fn tampered_ciphertext_fails() {
        let provider = test_key_provider();
        let envelope = test_envelope().await;

        let svc = CryptoService::new("test-key", provider);
        let mut encrypted = svc.encrypt(&envelope).await.expect("encrypt");

        let mut tampered = encrypted.ciphertext.to_vec();
        let last = tampered.len() - 1;
        tampered[last] ^= 0xFF;
        encrypted.ciphertext = Bytes::from(tampered);

        assert!(svc.decrypt(encrypted).await.is_err());
    }
}
