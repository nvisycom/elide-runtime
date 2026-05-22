//! Self-describing wire envelope for encrypted content.

use std::str;

use bytes::Bytes;
use nvisy_core::content::ContentSource;
use nvisy_core::{Error, Result};

use crate::workflow::EncryptionAlgorithm;

/// Wire-format magic bytes identifying an Nvisy encrypted blob.
const MAGIC: &[u8; 4] = b"NVSE";
/// Wire-format version.
const WIRE_VERSION: u8 = 0x01;
/// AES-256-GCM nonce size in bytes.
pub(crate) const NONCE_SIZE: usize = 12;
/// Minimum wire envelope size: magic(4) + version(1) + algo(1) + key_id_len(2) + nonce(12).
const MIN_HEADER_SIZE: usize = 4 + 1 + 1 + 2 + NONCE_SIZE;

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
}

/// Self-describing wire envelope.
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

        let key_id = str::from_utf8(&data[8..key_id_end]).map_err(|e| {
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
}
