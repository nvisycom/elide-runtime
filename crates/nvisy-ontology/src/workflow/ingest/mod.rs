//! Import and export node configurations.
//!
//! Ingest nodes form the boundary between the outside world and the pipeline.
//! [`ImportFile`] runs at **phase 0** to pull content in; [`ExportFile`] runs
//! at **phase 6** to push processed content out. Both nodes share the same
//! set of [`CompressionAlgorithm`] and [`EncryptionConfig`] codec options.

mod export;
mod import;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub use self::export::ExportFile;
pub use self::import::ImportFile;
use crate::Error;

/// Supported compression algorithms for import/export.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum CompressionAlgorithm {
    /// Gzip (.gz).
    Gzip,
    /// Zstandard (.zst).
    Zstd,
}

/// Supported encryption algorithms for import/export.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum EncryptionAlgorithm {
    /// AES-256 in Galois/Counter Mode.
    Aes256Gcm,
}

impl EncryptionAlgorithm {
    /// Encode as a single-byte wire tag.
    pub fn wire_tag(self) -> u8 {
        match self {
            Self::Aes256Gcm => 0x01,
        }
    }

    /// Decode from a single-byte wire tag.
    pub fn from_wire_tag(tag: u8) -> Result<Self, Error> {
        match tag {
            0x01 => Ok(Self::Aes256Gcm),
            _ => Err(Error::new(format!(
                "unknown encryption algorithm tag: 0x{tag:02x} (valid: 0x01 = aes256gcm)"
            ))),
        }
    }
}

/// Encryption configuration pairing an algorithm with a key identifier.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct EncryptionConfig {
    /// Algorithm to use for encryption/decryption.
    pub algorithm: EncryptionAlgorithm,
    /// Identifier of the key to resolve via the engine's `KeyProvider`.
    pub key_id: String,
}
