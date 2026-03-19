//! File export: deliver processed content to a downstream target.
//!
//! The export pipeline applies optional post-processing steps in order:
//!
//! 1. **Encryption** — encrypt content (if format specified)
//! 2. **Compression** — compress for storage or transfer (if format specified)

use nvisy_core::Result;

use crate::graph::{CompressionFormat, EncryptionFormat};
use crate::operation::{Operation, ParallelContext};

const TARGET: &str = "nvisy_engine::op::export_file";

/// Exports processed content, optionally applying encryption and
/// compression afterward.
pub struct ExportFile {
    encryption: Option<EncryptionFormat>,
    compression: Option<CompressionFormat>,
}

impl ExportFile {
    pub fn new() -> Self {
        Self {
            encryption: None,
            compression: None,
        }
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
            return Err(nvisy_core::Error::runtime(
                format!("export encryption ({format:?}) is not yet implemented"),
                "export_file",
                false,
            ));
        }
        if let Some(format) = self.compression {
            return Err(nvisy_core::Error::runtime(
                format!("export compression ({format:?}) is not yet implemented"),
                "export_file",
                false,
            ));
        }
        tracing::debug!(target: TARGET, "exporting content");
        Ok(())
    }
}

impl Default for ExportFile {
    fn default() -> Self {
        Self::new()
    }
}

impl Operation for ExportFile {
    type Input = ParallelContext;
    type Output = ParallelContext;

    async fn call(&self, input: Self::Input) -> Result<Self::Output> {
        input.parallel_map(|data| self.export(data)).await
    }
}
