//! [`Encrypt`]: reversible AES-256-GCM redaction.
//!
//! Output wire shape: `base64(nonce || ciphertext || auth_tag)`,
//! where `nonce` is a fresh 12-byte random value per call.
//!
//! The nonce is prepended so [`Decrypt`] can parse it without
//! out-of-band metadata; the auth tag is appended by `aes-gcm` and
//! verified on decryption. The whole blob is base64-encoded to keep
//! the substituted payload printable.
//!
//! Inverse lives in [`Decrypt`] (sibling module).
//!
//! [`Decrypt`]: crate::redaction::deanonymizer::Decrypt

use aes_gcm::aead::rand_core::RngCore;
use aes_gcm::aead::{Aead, KeyInit, OsRng};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use nvisy_core::entity::Entity;
use nvisy_core::modality::{Tabular, Text, TextData};
use nvisy_core::{Error, Result};

use crate::redaction::{Anonymizer, LeakProfile, TabularReplacement, TextReplacement};

/// Nonce length in bytes (AES-GCM standard). Shared with
/// [`crate::redaction::deanonymizer::Decrypt`] so both sides of
/// the AES-256-GCM envelope agree on the nonce prefix.
pub(crate) const NONCE_LEN: usize = 12;

const TARGET: &str = "nvisy_toolkit::redaction::encrypt";

/// AES-256-GCM encrypt operator.
///
/// Holds a 32-byte key. Use [`Decrypt`] with the same key to recover
/// the originals from an audit trail.
///
/// [`Decrypt`]: crate::redaction::deanonymizer::Decrypt
#[derive(Clone)]
pub struct Encrypt {
    key: Key<Aes256Gcm>,
}

impl Encrypt {
    /// Construct from a 32-byte key.
    pub fn from_key(key: [u8; 32]) -> Self {
        Self { key: key.into() }
    }

    /// Encrypt `value` and return the base64-encoded
    /// `nonce || ciphertext || auth_tag` blob.
    fn encrypt_value(&self, value: &str) -> Result<String> {
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

        Ok(STANDARD.encode(&blob))
    }
}

#[async_trait::async_trait]
impl Anonymizer<Text> for Encrypt {
    fn leak_profile(&self) -> LeakProfile {
        LeakProfile::Recoverable
    }

    async fn apply(&self, _entity: &Entity<Text>, source: &TextData) -> Result<TextReplacement> {
        Ok(TextReplacement::substituted(
            self.encrypt_value(source.text.as_str())?,
        ))
    }
}

#[async_trait::async_trait]
impl Anonymizer<Tabular> for Encrypt {
    fn leak_profile(&self) -> LeakProfile {
        LeakProfile::Recoverable
    }

    async fn apply(
        &self,
        _entity: &Entity<Tabular>,
        source: &TextData,
    ) -> Result<TabularReplacement> {
        Ok(TabularReplacement::substituted(
            self.encrypt_value(source.text.as_str())?,
        ))
    }
}

#[cfg(test)]
mod tests {
    use nvisy_core::entity::{TrailStep, builtins};
    use nvisy_core::modality::TextLocation;
    use nvisy_core::primitive::Confidence;

    use super::*;

    pub(super) const KEY: [u8; 32] = [
        0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e, 0x0f,
        0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1a, 0x1b, 0x1c, 0x1d, 0x1e,
        0x1f, 0x20,
    ];

    fn entity(start: usize, end: usize) -> Entity<Text> {
        Entity::builder()
            .with_label(builtins::EMAIL_ADDRESS.label_ref())
            .with_location(TextLocation::new(start, end))
            .with_confidence(Confidence::new(1.0).unwrap())
            .with_trail(Vec::<TrailStep>::new())
            .build()
            .unwrap()
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
}
