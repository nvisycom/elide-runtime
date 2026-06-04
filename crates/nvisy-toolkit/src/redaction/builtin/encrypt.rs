//! [`Encrypt`] + [`Decrypt`]: reversible AES-256-GCM redaction.
//!
//! Output wire shape: `base64(nonce || ciphertext || auth_tag)`,
//! where `nonce` is a fresh 12-byte random value per call.
//!
//! The nonce is prepended so `Decrypt` can parse it without
//! out-of-band metadata; the auth tag is appended by `aes-gcm` and
//! verified on decryption. The whole blob is base64-encoded to keep
//! the [`TextReplacement::Substituted`] payload printable.

use aes_gcm::aead::rand_core::RngCore;
use aes_gcm::aead::{Aead, KeyInit, OsRng};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use async_trait::async_trait;
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use nvisy_core::entity::Entity;
use nvisy_core::modality::{Text, TextData};
use nvisy_core::{Error, Result};

use super::text_value::read_value;
use crate::redaction::{Anonymizer, Deanonymizer, LeakProfile, TextReplacement};

/// Nonce length in bytes (AES-GCM standard).
const NONCE_LEN: usize = 12;

const TARGET: &str = "nvisy_toolkit::redaction::encrypt";

/// AES-256-GCM encrypt operator.
///
/// Holds a 32-byte key. Use [`Decrypt`] with the same key to recover
/// the originals from an audit trail.
#[derive(Clone)]
pub struct Encrypt {
    key: Key<Aes256Gcm>,
}

impl Encrypt {
    /// Construct from a 32-byte key.
    pub fn from_key(key: [u8; 32]) -> Self {
        Self { key: key.into() }
    }
}

/// AES-256-GCM decrypt operator. Inverse of [`Encrypt`].
#[derive(Clone)]
pub struct Decrypt {
    key: Key<Aes256Gcm>,
}

impl Decrypt {
    /// Construct from the same 32-byte key passed to [`Encrypt`].
    pub fn from_key(key: [u8; 32]) -> Self {
        Self { key: key.into() }
    }
}

#[async_trait]
impl Anonymizer<Text> for Encrypt {
    fn leak_profile(&self) -> LeakProfile {
        LeakProfile::Recoverable
    }

    async fn apply(&self, entity: &Entity<Text>, source: &TextData) -> Result<TextReplacement> {
        let value = read_value(entity, source);
        let cipher = Aes256Gcm::new(&self.key);

        let mut nonce_bytes = [0u8; NONCE_LEN];
        OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        let ciphertext = cipher
            .encrypt(nonce, value.as_bytes())
            .map_err(|e| Error::runtime(format!("encryption failed: {e}"), TARGET, false))?;

        let mut blob = Vec::with_capacity(NONCE_LEN + ciphertext.len());
        blob.extend_from_slice(&nonce_bytes);
        blob.extend_from_slice(&ciphertext);

        Ok(TextReplacement::substituted(STANDARD.encode(&blob)))
    }
}

#[async_trait]
impl Deanonymizer<Text> for Decrypt {
    async fn revert(&self, replacement: &TextReplacement) -> Result<TextData> {
        let TextReplacement::Substituted { value } = replacement else {
            return Err(Error::validation(
                "cannot decrypt: replacement is Removed (no ciphertext)",
                TARGET,
            ));
        };

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

#[cfg(test)]
mod tests {
    use nvisy_core::entity::{EntityKind, TrailStep};
    use nvisy_core::modality::TextLocation;
    use nvisy_core::primitive::Confidence;

    use super::*;

    const KEY: [u8; 32] = [
        0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e, 0x0f,
        0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1a, 0x1b, 0x1c, 0x1d, 0x1e,
        0x1f, 0x20,
    ];

    fn entity(start: usize, end: usize) -> Entity<Text> {
        Entity::builder()
            .with_entity_kind(EntityKind::EmailAddress)
            .with_location(TextLocation::new(start, end))
            .with_confidence(Confidence::new(1.0).unwrap())
            .with_trail(Vec::<TrailStep>::new())
            .build()
            .unwrap()
    }

    #[tokio::test]
    async fn encrypt_then_decrypt_round_trips() {
        let enc = Encrypt::from_key(KEY);
        let dec = Decrypt::from_key(KEY);
        let source = TextData::new("alice@example.test");
        let entity = entity(0, 18);

        let ciphertext = enc.apply(&entity, &source).await.unwrap();
        let recovered = dec.revert(&ciphertext).await.unwrap();
        assert_eq!(recovered.text.as_str(), "alice@example.test");
    }

    #[tokio::test]
    async fn ciphertext_differs_each_call() {
        let enc = Encrypt::from_key(KEY);
        let source = TextData::new("alice@example.test");
        let entity = entity(0, 18);
        let a = enc.apply(&entity, &source).await.unwrap();
        let b = enc.apply(&entity, &source).await.unwrap();
        assert_ne!(a, b, "fresh nonce must produce different ciphertext");
    }

    #[tokio::test]
    async fn wrong_key_fails_decryption() {
        let enc = Encrypt::from_key(KEY);
        let mut bad_key = KEY;
        bad_key[0] ^= 0xff;
        let dec = Decrypt::from_key(bad_key);
        let source = TextData::new("alice@example.test");
        let entity = entity(0, 18);

        let ciphertext = enc.apply(&entity, &source).await.unwrap();
        let err = dec.revert(&ciphertext).await.unwrap_err();
        assert!(err.to_string().contains("decryption failed"));
    }

    #[tokio::test]
    async fn removed_replacement_cannot_be_decrypted() {
        let dec = Decrypt::from_key(KEY);
        let err = dec.revert(&TextReplacement::Removed).await.unwrap_err();
        assert!(err.to_string().contains("Removed"));
    }
}
