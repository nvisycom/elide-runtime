//! Cryptographic primitives: key providers, wire format, encrypt/decrypt.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use aes_gcm::aead::Aead;
use aes_gcm::{Aes256Gcm, KeyInit, Nonce};
use bytes::Bytes;
use nvisy_core::content::{ContentData, ContentSource};
use nvisy_core::{Error, ErrorKind, Result};
use rand::RngExt;

use crate::operation::DocumentEnvelope;

/// Wire-format magic bytes identifying an Nvisy encrypted blob.
const MAGIC: &[u8; 4] = b"NVSE";
/// Wire-format version.
const WIRE_VERSION: u8 = 0x01;
/// AES-256-GCM nonce size in bytes.
pub(crate) const NONCE_SIZE: usize = 12;
/// Minimum wire envelope size: magic(4) + version(1) + algo(1) + key_id_len(2) + nonce(12).
const MIN_HEADER_SIZE: usize = 4 + 1 + 1 + 2 + NONCE_SIZE;

const TARGET: &str = "nvisy_engine::crypto";

/// Supported encryption algorithms.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EncryptionAlgorithm {
    /// AES-256 in Galois/Counter Mode.
    Aes256Gcm,
}

impl EncryptionAlgorithm {
    fn wire_tag(self) -> u8 {
        match self {
            Self::Aes256Gcm => 0x01,
        }
    }

    fn from_wire_tag(tag: u8) -> Result<Self> {
        match tag {
            0x01 => Ok(Self::Aes256Gcm),
            _ => Err(Error::validation(
                format!("unknown encryption algorithm tag: 0x{tag:02x}"),
                "EncryptionAlgorithm::from_wire_tag",
            )),
        }
    }
}

/// Encrypted content blob with metadata.
#[derive(Debug, Clone)]
pub struct EncryptedContent {
    /// Source identity of the original content.
    pub source: ContentSource,
    /// Self-describing wire-format blob (header + ciphertext).
    pub ciphertext: Bytes,
    /// Identifier of the key used for encryption.
    pub key_id: String,
    /// Algorithm used for encryption.
    pub algorithm: EncryptionAlgorithm,
    /// Original filename, if any.
    pub filename: Option<PathBuf>,
}

/// Abstraction for resolving encryption keys by identifier.
pub trait KeyProvider: Send + Sync {
    /// Returns the raw key bytes for the given `key_id`, or an error if unknown.
    fn resolve(&self, key_id: &str) -> Result<Vec<u8>>;
}

/// In-memory key store for tests and simple deployments.
#[derive(Debug, Clone)]
pub struct StaticKeyProvider {
    keys: HashMap<String, Vec<u8>>,
}

impl StaticKeyProvider {
    /// Creates a new provider from an iterator of `(key_id, key_bytes)` pairs.
    pub fn new(keys: impl IntoIterator<Item = (String, Vec<u8>)>) -> Self {
        Self {
            keys: keys.into_iter().collect(),
        }
    }
}

impl KeyProvider for StaticKeyProvider {
    fn resolve(&self, key_id: &str) -> Result<Vec<u8>> {
        self.keys.get(key_id).cloned().ok_or_else(|| {
            Error::validation(
                format!("unknown key_id: {key_id}"),
                "StaticKeyProvider::resolve",
            )
        })
    }
}

/// AES-256-GCM encryption and decryption service.
pub struct CryptoService {
    key_id: String,
    key_provider: Arc<dyn KeyProvider>,
}

impl CryptoService {
    /// Creates a new crypto service.
    pub fn new(key_id: impl Into<String>, key_provider: Arc<dyn KeyProvider>) -> Self {
        Self {
            key_id: key_id.into(),
            key_provider,
        }
    }

    /// Encrypt a [`DocumentEnvelope`] into an [`EncryptedContent`] blob.
    pub async fn encrypt(&self, envelope: &DocumentEnvelope) -> Result<EncryptedContent> {
        let content_data = envelope.document.encode()?;
        let source = content_data.content_source;
        let filename = content_data.filename.clone();
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
            filename,
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

        let mut content = ContentData::new(encrypted.source, Bytes::from(plaintext));
        if let Some(filename) = encrypted.filename {
            content = content.with_filename(filename);
        }

        Ok(content)
    }
}

/// Self-describing wire envelope for encrypted content.
///
/// Wire format:
/// ```text
/// [4B magic "NVSE"] [1B version] [1B algo]
/// [2B key_id len BE] [N bytes key_id UTF-8]
/// [12B nonce] [ciphertext + 16B GCM tag]
/// ```
struct WireEnvelope<'a> {
    algorithm: EncryptionAlgorithm,
    key_id: &'a str,
    nonce: &'a [u8; NONCE_SIZE],
    ciphertext: &'a [u8],
}

impl WireEnvelope<'_> {
    fn build(&self) -> Bytes {
        let key_id_bytes = self.key_id.as_bytes();
        let key_id_len = key_id_bytes.len() as u16;
        let total = MIN_HEADER_SIZE + key_id_bytes.len() + self.ciphertext.len();

        let mut buf = Vec::with_capacity(total);
        buf.extend_from_slice(MAGIC);
        buf.push(WIRE_VERSION);
        buf.push(self.algorithm.wire_tag());
        buf.extend_from_slice(&key_id_len.to_be_bytes());
        buf.extend_from_slice(key_id_bytes);
        buf.extend_from_slice(self.nonce);
        buf.extend_from_slice(self.ciphertext);

        Bytes::from(buf)
    }

    fn parse(data: &[u8]) -> Result<WireEnvelope<'_>> {
        if data.len() < MIN_HEADER_SIZE {
            return Err(Error::validation(
                "wire envelope too short",
                "WireEnvelope::parse",
            ));
        }

        if &data[..4] != MAGIC {
            return Err(Error::validation(
                "invalid magic bytes",
                "WireEnvelope::parse",
            ));
        }

        let version = data[4];
        if version != WIRE_VERSION {
            return Err(Error::validation(
                format!("unsupported wire version: {version}"),
                "WireEnvelope::parse",
            ));
        }

        let algorithm = EncryptionAlgorithm::from_wire_tag(data[5])?;
        let key_id_len = u16::from_be_bytes([data[6], data[7]]) as usize;

        let key_id_end = 8 + key_id_len;
        let nonce_end = key_id_end + NONCE_SIZE;

        if data.len() < nonce_end {
            return Err(Error::validation(
                "wire envelope truncated: missing key_id or nonce",
                "WireEnvelope::parse",
            ));
        }

        let key_id = std::str::from_utf8(&data[8..key_id_end]).map_err(|e| {
            Error::validation(format!("invalid key_id UTF-8: {e}"), "WireEnvelope::parse")
        })?;

        let nonce: &[u8; NONCE_SIZE] = data[key_id_end..nonce_end].try_into().unwrap();
        let ciphertext = &data[nonce_end..];

        Ok(WireEnvelope {
            algorithm,
            key_id,
            nonce,
            ciphertext,
        })
    }
}

#[cfg(test)]
mod tests {
    use nvisy_codec::Document;

    use super::*;

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

    #[test]
    fn wire_format_round_trip() {
        let nonce = [1u8; NONCE_SIZE];
        let ciphertext = b"some-ciphertext-data";
        let key_id = "my-key-id";

        let wire = WireEnvelope {
            algorithm: EncryptionAlgorithm::Aes256Gcm,
            key_id,
            nonce: &nonce,
            ciphertext,
        }
        .build();

        let parsed = WireEnvelope::parse(&wire).expect("parse");
        assert_eq!(parsed.algorithm, EncryptionAlgorithm::Aes256Gcm);
        assert_eq!(parsed.key_id, key_id);
        assert_eq!(parsed.nonce, &nonce);
        assert_eq!(parsed.ciphertext, ciphertext);
    }

    #[test]
    fn wire_format_truncated_header() {
        assert!(WireEnvelope::parse(b"NVS").is_err());
    }

    #[test]
    fn wire_format_bad_magic() {
        let mut data = vec![0u8; MIN_HEADER_SIZE];
        data[..4].copy_from_slice(b"BAAD");
        assert!(WireEnvelope::parse(&data).is_err());
    }

    #[test]
    fn wire_format_truncated_key_id() {
        let mut data = vec![0u8; MIN_HEADER_SIZE];
        data[..4].copy_from_slice(MAGIC);
        data[4] = WIRE_VERSION;
        data[5] = 0x01;
        data[6..8].copy_from_slice(&100u16.to_be_bytes());
        assert!(WireEnvelope::parse(&data).is_err());
    }

    #[tokio::test]
    async fn round_trip_encrypt_decrypt() {
        let provider = test_key_provider();
        let envelope = test_envelope().await;
        let original_bytes = envelope.document.encode().expect("encode").into_bytes();

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

        let wrong_provider = Arc::new(StaticKeyProvider::new([(
            "test-key".to_string(),
            vec![0xCD; 32],
        )]));
        let dec_svc = CryptoService::new("test-key", wrong_provider);
        assert!(dec_svc.decrypt(encrypted).await.is_err());
    }

    #[tokio::test]
    async fn unknown_key_id_fails() {
        let empty_provider = Arc::new(StaticKeyProvider::new([]));
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
