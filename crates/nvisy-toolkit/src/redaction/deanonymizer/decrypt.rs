//! [`Decrypt`]: inverse of [`Encrypt`].
//!
//! Implements [`Deanonymizer<M>`] as a **self-contained** recovery
//! operator: the ciphertext blob in the replacement carries
//! everything needed to decode it, so the [`Entity`] argument is
//! ignored. See the [`Deanonymizer`] module docs for the audit-
//! keyed vs. self-contained distinction.
//!
//! Wire shape matches what [`Encrypt`] produces:
//! `base64(nonce || ciphertext || auth_tag)` with a 12-byte nonce.
//!
//! [`Encrypt`]: crate::redaction::anonymizer::Encrypt
//! [`Deanonymizer`]: nvisy_core::redaction::Deanonymizer
//! [`Deanonymizer<M>`]: nvisy_core::redaction::Deanonymizer

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use nvisy_core::entity::Entity;
use nvisy_core::modality::{Tabular, Text, TextData};
use nvisy_core::redaction::{Deanonymizer, TabularReplacement, TextReplacement};
use nvisy_core::{Error, Result};

use crate::redaction::anonymizer::encrypt::NONCE_LEN;

const TARGET: &str = "nvisy_toolkit::redaction::decrypt";

/// AES-256-GCM decrypt operator. Inverse of [`Encrypt`].
///
/// [`Encrypt`]: crate::redaction::anonymizer::Encrypt
#[derive(Clone)]
pub struct Decrypt {
    key: Key<Aes256Gcm>,
}

impl Decrypt {
    /// Construct from the same 32-byte key passed to [`Encrypt`].
    ///
    /// [`Encrypt`]: crate::redaction::anonymizer::Encrypt
    pub fn from_key(key: [u8; 32]) -> Self {
        Self { key: key.into() }
    }

    /// Parse the base64 envelope, split nonce from ciphertext, run
    /// AES-256-GCM, and decode the plaintext bytes as UTF-8.
    fn decrypt_blob(&self, value: &str) -> Result<TextData> {
        let blob = STANDARD
            .decode(value)
            .map_err(|e| Error::validation(format!("invalid base64 ciphertext: {e}"), TARGET))?;

        if blob.len() < NONCE_LEN {
            return Err(Error::validation(
                format!("ciphertext too short for nonce (got {} bytes)", blob.len()),
                TARGET,
            ));
        }

        let (nonce_bytes, ciphertext) = blob.split_at(NONCE_LEN);
        let nonce = Nonce::from_slice(nonce_bytes);
        let cipher = Aes256Gcm::new(&self.key);
        let plaintext = cipher.decrypt(nonce, ciphertext).map_err(|e| {
            Error::runtime(
                format!("decryption failed (wrong key or tampered): {e}"),
                TARGET,
                false,
            )
        })?;

        let text = String::from_utf8(plaintext).map_err(|e| {
            Error::validation(format!("decrypted bytes are not valid UTF-8: {e}"), TARGET)
        })?;

        Ok(TextData::new(text))
    }
}

#[async_trait::async_trait]
impl Deanonymizer<Text> for Decrypt {
    async fn revert(
        &self,
        _entity: &Entity<Text>,
        replacement: &TextReplacement,
    ) -> Result<Option<TextData>> {
        let TextReplacement::Substituted { value } = replacement else {
            return Ok(None);
        };
        self.decrypt_blob(value).map(Some)
    }
}

#[async_trait::async_trait]
impl Deanonymizer<Tabular> for Decrypt {
    async fn revert(
        &self,
        _entity: &Entity<Tabular>,
        replacement: &TabularReplacement,
    ) -> Result<Option<TextData>> {
        let TabularReplacement::Substituted { value } = replacement else {
            return Ok(None);
        };
        self.decrypt_blob(value).map(Some)
    }
}

#[cfg(test)]
mod tests {
    use nvisy_core::entity::{TrailStep, builtins};
    use nvisy_core::modality::{TabularLocation, TextLocation};
    use nvisy_core::primitive::Confidence;

    use super::*;
    use crate::redaction::Anonymizer;
    use crate::redaction::anonymizer::Encrypt;

    const KEY: [u8; 32] = [
        0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e, 0x0f,
        0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1a, 0x1b, 0x1c, 0x1d, 0x1e,
        0x1f, 0x20,
    ];

    fn text_entity(start: usize, end: usize) -> Entity<Text> {
        Entity::builder()
            .with_label(builtins::EMAIL_ADDRESS.label_ref())
            .with_location(TextLocation::new(start, end))
            .with_confidence(Confidence::new(1.0).unwrap())
            .with_trail(Vec::<TrailStep>::new())
            .build()
            .unwrap()
    }

    fn tabular_entity(row: u32, col: u32) -> Entity<Tabular> {
        Entity::builder()
            .with_label(builtins::EMAIL_ADDRESS.label_ref())
            .with_location(TabularLocation::new(row, col))
            .with_confidence(Confidence::new(1.0).unwrap())
            .with_trail(Vec::<TrailStep>::new())
            .build()
            .unwrap()
    }

    #[tokio::test]
    async fn text_round_trip() {
        let enc = Encrypt::from_key(KEY);
        let dec = Decrypt::from_key(KEY);
        let source = TextData::new("alice@example.test");
        let entity = text_entity(0, 18);

        let ciphertext = enc.apply(&entity, &source).await.unwrap();
        let recovered = dec.revert(&entity, &ciphertext).await.unwrap();
        assert_eq!(
            recovered.expect("decrypts cleanly").text.as_str(),
            "alice@example.test"
        );
    }

    #[tokio::test]
    async fn tabular_round_trip() {
        let enc = Encrypt::from_key(KEY);
        let dec = Decrypt::from_key(KEY);
        let source = TextData::new("alice@example.test");
        let entity = tabular_entity(1, 1);

        let ciphertext: TabularReplacement = enc.apply(&entity, &source).await.unwrap();
        let recovered = dec.revert(&entity, &ciphertext).await.unwrap();
        assert_eq!(
            recovered.expect("decrypts cleanly").text.as_str(),
            "alice@example.test"
        );
    }

    #[tokio::test]
    async fn wrong_key_fails_decryption() {
        let enc = Encrypt::from_key(KEY);
        let mut bad_key = KEY;
        bad_key[0] ^= 0xff;
        let dec = Decrypt::from_key(bad_key);
        let source = TextData::new("alice@example.test");
        let entity = text_entity(0, 18);

        let ciphertext = enc.apply(&entity, &source).await.unwrap();
        let err = dec.revert(&entity, &ciphertext).await.unwrap_err();
        assert!(err.to_string().contains("decryption failed"));
    }

    #[tokio::test]
    async fn removed_text_replacement_yields_none() {
        let dec = Decrypt::from_key(KEY);
        let entity = text_entity(0, 0);
        let out = dec
            .revert(&entity, &TextReplacement::Removed)
            .await
            .unwrap();
        assert!(out.is_none());
    }

    #[tokio::test]
    async fn column_dropped_tabular_replacement_yields_none() {
        let dec = Decrypt::from_key(KEY);
        let entity = tabular_entity(1, 1);
        let out = dec
            .revert(&entity, &TabularReplacement::ColumnDropped)
            .await
            .unwrap();
        assert!(out.is_none());
    }
}
