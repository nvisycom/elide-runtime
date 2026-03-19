//! File export: deliver processed content to a downstream target.
//!
//! The export pipeline applies optional post-processing steps in order:
//!
//! 1. **Encryption** — encrypt content (if `encryption` is set)
//! 2. **Compression** — compress for storage or transfer (if `compression` is set)

use nvisy_core::Result;

use crate::operation::{Operation, ParallelContext};

const TARGET: &str = "nvisy_engine::op::export_file";

/// Exports processed content, optionally applying encryption and
/// compression afterward.
pub struct ExportFile {
    encryption: bool,
    compression: bool,
}

impl ExportFile {
    /// Create a new export operation with default settings (no post-processing).
    pub fn new() -> Self {
        Self {
            encryption: false,
            compression: false,
        }
    }

    /// Enable encryption before export.
    pub fn with_encryption(mut self, enabled: bool) -> Self {
        self.encryption = enabled;
        self
    }

    /// Enable compression before export.
    pub fn with_compression(mut self, enabled: bool) -> Self {
        self.compression = enabled;
        self
    }

    async fn export(&self, _data: ()) -> Result<()> {
        if self.encryption {
            return Err(nvisy_core::Error::runtime(
                "export encryption is not yet implemented",
                "export_file",
                false,
            ));
        }
        if self.compression {
            return Err(nvisy_core::Error::runtime(
                "export compression is not yet implemented",
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
