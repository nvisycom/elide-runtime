//! Content encryption operation.

use std::sync::Arc;

use aes_gcm::aead::Aead;
use aes_gcm::{Aes256Gcm, KeyInit, Nonce};
use nvisy_core::{Error, ErrorKind, Result};
use rand::RngExt;

use crate::operation::context::DocumentEnvelope;
use crate::operation::utility::crypto::{
    EncryptedContent, EncryptionAlgorithm, KeyProvider, NONCE_SIZE, WireEnvelope,
};
use crate::operation::{Operation, ParallelContext};

const TARGET: &str = "nvisy_engine::op::encryption";

/// Encrypts document content using AES-256-GCM.
///
/// Takes a [`DocumentEnvelope`] (post-redaction) and produces an
/// [`EncryptedContent`] containing the self-describing wire envelope.
pub struct Encryption {
    key_id: String,
    key_provider: Arc<dyn KeyProvider>,
}

impl Encryption {
    /// Creates a new encryption operation.
    ///
    /// - `key_id`: identifier of the key to encrypt with.
    /// - `key_provider`: resolves `key_id` to raw key bytes.
    pub fn new(key_id: impl Into<String>, key_provider: Arc<dyn KeyProvider>) -> Self {
        Self {
            key_id: key_id.into(),
            key_provider,
        }
    }

    async fn encrypt(&self, envelope: DocumentEnvelope) -> Result<EncryptedContent> {
        let content_data = envelope.document.encode()?;
        let source = content_data.content_source;
        let filename = content_data.filename.clone();
        let plaintext = content_data.as_bytes();

        let raw_key = self.key_provider.resolve(&self.key_id)?;
        let cipher = Aes256Gcm::new_from_slice(&raw_key).map_err(|e| {
            Error::validation(format!("invalid AES-256 key: {e}"), "Encryption::encrypt")
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
            filename,
        })
    }
}

impl Operation for Encryption {
    type Input = ParallelContext<DocumentEnvelope>;
    type Output = ParallelContext<EncryptedContent>;

    async fn call(&self, input: Self::Input) -> Result<Self::Output> {
        input.parallel_map(|data| self.encrypt(data)).await
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use bytes::Bytes;
    use nvisy_codec::Document;
    use nvisy_core::content::{ContentData, ContentSource};

    use super::*;
    use crate::operation::utility::crypto::StaticKeyProvider;

    fn test_key_provider() -> Arc<StaticKeyProvider> {
        let key = vec![0xAB; 32];
        Arc::new(StaticKeyProvider::new([("test-key".to_string(), key)]))
    }

    async fn test_envelope() -> DocumentEnvelope {
        let content = ContentData::from_text(ContentSource::new(), "Hello, world!")
            .with_content_type("text/plain");
        let doc = Document::decode(&content).await.expect("decode text");
        DocumentEnvelope::new(doc)
    }

    #[tokio::test]
    async fn round_trip_encrypt_decrypt() {
        let provider = test_key_provider();
        let envelope = test_envelope().await;

        let original_bytes = envelope.document.encode().expect("encode").into_bytes();

        let enc = Encryption::new("test-key", provider.clone());
        let encrypted = enc.encrypt(envelope).await.expect("encrypt");

        assert_eq!(encrypted.key_id, "test-key");
        assert_eq!(encrypted.algorithm, EncryptionAlgorithm::Aes256Gcm);
        assert_ne!(encrypted.ciphertext, original_bytes);

        let dec = crate::operation::lifecycle::Decryption::new(provider);
        let decrypted = dec.decrypt(encrypted).await.expect("decrypt");

        assert_eq!(decrypted.as_bytes(), &original_bytes[..]);
    }

    #[tokio::test]
    async fn wrong_key_fails_decryption() {
        let provider = test_key_provider();
        let envelope = test_envelope().await;

        let enc = Encryption::new("test-key", provider);
        let encrypted = enc.encrypt(envelope).await.expect("encrypt");

        let wrong_provider = Arc::new(StaticKeyProvider::new([(
            "test-key".to_string(),
            vec![0xCD; 32],
        )]));

        let dec = crate::operation::lifecycle::Decryption::new(wrong_provider);
        let result = dec.decrypt(encrypted).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn unknown_key_id_fails() {
        let empty_provider = Arc::new(StaticKeyProvider::new([]));
        let envelope = test_envelope().await;

        let enc = Encryption::new("nonexistent", empty_provider);
        let result = enc.encrypt(envelope).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn tampered_ciphertext_fails() {
        let provider = test_key_provider();
        let envelope = test_envelope().await;

        let enc = Encryption::new("test-key", provider.clone());
        let mut encrypted = enc.encrypt(envelope).await.expect("encrypt");

        let mut tampered = encrypted.ciphertext.to_vec();
        let last = tampered.len() - 1;
        tampered[last] ^= 0xFF;
        encrypted.ciphertext = Bytes::from(tampered);

        let dec = crate::operation::lifecycle::Decryption::new(provider);
        let result = dec.decrypt(encrypted).await;
        assert!(result.is_err());
    }
}
