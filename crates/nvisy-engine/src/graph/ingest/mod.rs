//! Import and export node configurations.
//!
//! Ingest nodes form the boundary between the outside world and the pipeline.
//! [`ImportFile`] runs at **phase 0** to pull content in; [`ExportFile`] runs
//! at **phase 6** to push processed content out. Both nodes share the same
//! set of [`CompressionFormat`] and [`EncryptionFormat`] codec options.

mod export;
mod import;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub use self::export::ExportFile;
pub use self::import::ImportFile;

/// Supported compression formats for import/export.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CompressionFormat {
    /// Gzip (.gz).
    Gzip,
    /// Zstandard (.zst).
    Zstd,
}

/// Supported encryption formats for import/export.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum EncryptionFormat {
    /// AES-256 in Galois/Counter Mode.
    Aes256Gcm,
}
