//! Content compression and decompression utilities.
//!
//! Used as pre/post-processing steps within [`ImportFile`] and
//! [`ExportFile`] operations, not as standalone pipeline operations.
//!
//! [`ImportFile`]: crate::operation::lifecycle::ImportFile
//! [`ExportFile`]: crate::operation::lifecycle::ExportFile

use bytes::Bytes;
use nvisy_core::{Error, Result};

const TARGET: &str = "nvisy_engine::compression";

/// Supported compression algorithms.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompressionAlgorithm {
    /// Gzip compression.
    Gzip,
    /// Zstandard compression.
    Zstd,
}

/// Compress raw bytes using the specified algorithm.
pub fn compress(data: &[u8], algorithm: CompressionAlgorithm) -> Result<Bytes> {
    tracing::debug!(
        target: TARGET,
        algorithm = ?algorithm,
        input_len = data.len(),
        "compressing content",
    );
    let _ = (data, algorithm);
    Err(Error::runtime(
        format!("compression ({algorithm:?}) not yet implemented"),
        "compress",
        false,
    ))
}

/// Decompress raw bytes using the specified algorithm.
pub fn decompress(data: &[u8], algorithm: CompressionAlgorithm) -> Result<Bytes> {
    tracing::debug!(
        target: TARGET,
        algorithm = ?algorithm,
        input_len = data.len(),
        "decompressing content",
    );
    let _ = (data, algorithm);
    Err(Error::runtime(
        format!("decompression ({algorithm:?}) not yet implemented"),
        "decompress",
        false,
    ))
}
