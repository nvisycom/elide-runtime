//! File export operation.
//!
//!
//! Runs at **phase 6** alongside [`SaveContext`]. Delivers processed
//! content, optionally applying encryption and compression.
//!
//! [`SaveContext`]: crate::operation::SaveContext

//! The export pipeline applies optional post-processing steps in order:
//!
//! 1. **Encryption** — encrypt content (if format specified)
//! 2. **Compression** — compress for storage or transfer (if format specified)

use nvisy_core::Result;

use crate::graph::{CompressionFormat, EncryptionFormat};
use crate::operation::compression::{self, CompressionAlgorithm};
use crate::operation::Operation;
use crate::operation::context::ParallelContext;

const TARGET: &str = "nvisy_engine::op::export_file";

/// Exports processed content, optionally applying encryption and
/// compression afterward.
#[derive(Default)]
pub struct ExportFile {
    encryption: Option<EncryptionFormat>,
    compression: Option<CompressionFormat>,
}

impl ExportFile {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_encryption(mut self, format: Option<EncryptionFormat>) -> Self {
        self.encryption = format;
        self
    }

    pub fn with_compression(mut self, format: Option<CompressionFormat>) -> Self {
        self.compression = format;
        self
    }

    async fn export(&self, _data: ()) -> Result<()> {
        if let Some(format) = self.encryption {
            tracing::debug!(target: TARGET, ?format, "encryption requested but not yet wired");
            return Err(nvisy_core::Error::runtime(
                format!("export encryption ({format:?}) requires a KeyProvider — not yet wired"),
                "export_file",
                false,
            ));
        }

        if let Some(format) = self.compression {
            let algorithm = match format {
                CompressionFormat::Gzip => CompressionAlgorithm::Gzip,
                CompressionFormat::Zstd => CompressionAlgorithm::Zstd,
            };
            tracing::debug!(target: TARGET, ?format, "compressing export content");
            // Compression will apply to the actual content bytes once export
            // receives real data instead of ().
            let _ = compression::compress(&[], algorithm)?;
        }

        tracing::debug!(target: TARGET, "exporting content");
        Ok(())
    }
}

impl Operation for ExportFile {
    type Input = ParallelContext;
    type Output = ParallelContext;

    async fn call(&self, input: Self::Input) -> Result<Self::Output> {
        input.parallel_map(|data| self.export(data)).await
    }
}
