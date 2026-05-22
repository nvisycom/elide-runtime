//! Ingestion: the pipeline's edge — content + context I/O.
//!
//! These configs form the boundary between the outside world and the
//! pipeline. [`ImportFile`] pulls content in and [`ExportFile`]
//! pushes processed content out; both share the same
//! [`CompressionAlgorithm`] / [`EncryptionConfig`] codec options.
//! [`LoadContext`] loads reference-data contexts before any phase
//! runs.

pub(crate) mod compression;
pub mod encryption;
mod export;
mod exporter;
mod import;
mod importer;
mod load_context;
pub mod registry;

use nvisy_ontology::Error;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub use self::export::ExportFile;
pub(crate) use self::exporter::Exporter;
pub use self::import::ImportFile;
pub(crate) use self::importer::Importer;
pub use self::load_context::LoadContext;

/// Supported compression algorithms for import/export.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
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
