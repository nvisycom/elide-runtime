//! Content compression and decompression.
//!
//! Used as pre/post-processing steps within `ImportFile` and
//! `ExportFile` operations, not as standalone pipeline operations.
//!
//! Gzip and Zstd are recognized but not yet implemented — selecting
//! them returns a runtime error.

use bytes::Bytes;
use nvisy_core::{Error, Result};

use crate::core::ingestion::CompressionAlgorithm;

const TARGET: &str = "nvisy_engine::op::compression";

/// Compression and decompression service.
///
/// Wraps a specific algorithm selected at construction time.
pub struct CompressionService {
    algorithm: CompressionAlgorithm,
}

impl CompressionService {
    /// Create a new service for the given algorithm.
    pub fn new(algorithm: CompressionAlgorithm) -> Self {
        Self { algorithm }
    }

    /// Compress raw bytes.
    pub fn compress(&self, data: &[u8]) -> Result<Bytes> {
        tracing::debug!(
            target: TARGET,
            algorithm = ?self.algorithm,
            input_len = data.len(),
            "compressing content",
        );
        match self.algorithm {
            CompressionAlgorithm::Gzip => Err(Error::runtime(
                "gzip compression not yet implemented",
                "compression",
                false,
            )),
            CompressionAlgorithm::Zstd => Err(Error::runtime(
                "zstd compression not yet implemented",
                "compression",
                false,
            )),
        }
    }

    /// Decompress raw bytes.
    pub fn decompress(&self, data: &[u8]) -> Result<Bytes> {
        tracing::debug!(
            target: TARGET,
            algorithm = ?self.algorithm,
            input_len = data.len(),
            "decompressing content",
        );
        match self.algorithm {
            CompressionAlgorithm::Gzip => Err(Error::runtime(
                "gzip decompression not yet implemented",
                "compression",
                false,
            )),
            CompressionAlgorithm::Zstd => Err(Error::runtime(
                "zstd decompression not yet implemented",
                "compression",
                false,
            )),
        }
    }
}
