//! Content decryption operation.

use std::sync::Arc;

use aes_gcm::aead::Aead;
use aes_gcm::{Aes256Gcm, KeyInit, Nonce};
use bytes::Bytes;
use nvisy_core::content::ContentData;
use nvisy_core::{Error, Result};

use crate::operation::utility::crypto::{
    EncryptedContent, EncryptionAlgorithm, KeyProvider, WireEnvelope,
};
use crate::operation::{Operation, ParallelContext};

const TARGET: &str = "nvisy_engine::op::decryption";

/// Decrypts content produced by [`super::Encryption`], yielding raw
/// [`ContentData`] suitable for re-import.
pub struct Decryption {
    key_provider: Arc<dyn KeyProvider>,
}

impl Decryption {
    /// Creates a new decryption operation.
    pub fn new(key_provider: Arc<dyn KeyProvider>) -> Self {
        Self { key_provider }
    }

    pub(crate) async fn decrypt(&self, encrypted: EncryptedContent) -> Result<ContentData> {
        let envelope = WireEnvelope::parse(&encrypted.ciphertext)?;

        if envelope.algorithm != EncryptionAlgorithm::Aes256Gcm {
            return Err(Error::validation(
                format!("unsupported algorithm: {:?}", envelope.algorithm),
                "Decryption::decrypt",
            ));
        }

        let raw_key = self.key_provider.resolve(envelope.key_id)?;

        let cipher = Aes256Gcm::new_from_slice(&raw_key).map_err(|e| {
            Error::validation(format!("invalid AES-256 key: {e}"), "Decryption::decrypt")
        })?;

        let nonce = Nonce::from_slice(envelope.nonce);
        let plaintext = cipher.decrypt(nonce, envelope.ciphertext).map_err(|e| {
            Error::validation(
                format!("AES-256-GCM decryption failed (authentication error): {e}"),
                "Decryption::decrypt",
            )
        })?;

        tracing::debug!(
            target: TARGET,
            key_id = %envelope.key_id,
            ciphertext_len = envelope.ciphertext.len(),
            plaintext_len = plaintext.len(),
            "decrypted content",
        );

        let mut content = ContentData::new(encrypted.source, Bytes::from(plaintext));
        if let Some(filename) = encrypted.filename {
            content = content.with_filename(filename);
        }

        Ok(content)
    }
}

impl Operation for Decryption {
    type Input = ParallelContext<EncryptedContent>;
    type Output = ParallelContext<ContentData>;

    async fn call(&self, input: Self::Input) -> Result<Self::Output> {
        input.parallel_map(|data| self.decrypt(data)).await
    }
}
