//! Cryptographic primitives: key providers, wire format, and shared types.

use std::collections::HashMap;
use std::path::PathBuf;

use bytes::Bytes;
use nvisy_core::content::ContentSource;
use nvisy_core::{Error, Result};

/// Wire-format magic bytes identifying an Nvisy encrypted blob.
pub(crate) const MAGIC: &[u8; 4] = b"NVSE";
/// Wire-format version.
pub(crate) const WIRE_VERSION: u8 = 0x01;
/// AES-256-GCM nonce size in bytes.
pub(crate) const NONCE_SIZE: usize = 12;
/// Minimum wire envelope size: magic(4) + version(1) + algo(1) + key_id_len(2) + nonce(12).
pub(crate) const MIN_HEADER_SIZE: usize = 4 + 1 + 1 + 2 + NONCE_SIZE;

/// Supported encryption algorithms.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EncryptionAlgorithm {
    /// AES-256 in Galois/Counter Mode.
    Aes256Gcm,
}

impl EncryptionAlgorithm {
    pub(crate) fn wire_tag(self) -> u8 {
        match self {
            Self::Aes256Gcm => 0x01,
        }
    }

    pub(crate) fn from_wire_tag(tag: u8) -> Result<Self> {
        match tag {
            0x01 => Ok(Self::Aes256Gcm),
            _ => Err(Error::validation(
                format!("unknown encryption algorithm tag: 0x{tag:02x}"),
                "EncryptionAlgorithm::from_wire_tag",
            )),
        }
    }
}

/// Encrypted content produced by the [`super::super::lifecycle::Encryption`] operation.
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

/// Self-describing wire envelope for encrypted content.
///
/// Wire format:
/// ```text
/// [4B magic "NVSE"] [1B version] [1B algo]
/// [2B key_id len BE] [N bytes key_id UTF-8]
/// [12B nonce] [ciphertext + 16B GCM tag]
/// ```
pub(crate) struct WireEnvelope<'a> {
    pub algorithm: EncryptionAlgorithm,
    pub key_id: &'a str,
    pub nonce: &'a [u8; NONCE_SIZE],
    pub ciphertext: &'a [u8],
}

impl WireEnvelope<'_> {
    /// Serializes this envelope into the self-describing wire format.
    pub fn build(&self) -> Bytes {
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

    /// Parses a wire envelope, returning references into the source buffer.
    pub fn parse(data: &[u8]) -> Result<WireEnvelope<'_>> {
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
    use super::*;

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
        let result = WireEnvelope::parse(b"NVS");
        assert!(result.is_err());
    }

    #[test]
    fn wire_format_bad_magic() {
        let mut data = vec![0u8; MIN_HEADER_SIZE];
        data[..4].copy_from_slice(b"BAAD");
        let result = WireEnvelope::parse(&data);
        assert!(result.is_err());
    }

    #[test]
    fn wire_format_truncated_key_id() {
        let mut data = vec![0u8; MIN_HEADER_SIZE];
        data[..4].copy_from_slice(MAGIC);
        data[4] = WIRE_VERSION;
        data[5] = 0x01;
        data[6..8].copy_from_slice(&100u16.to_be_bytes());
        let result = WireEnvelope::parse(&data);
        assert!(result.is_err());
    }
}
