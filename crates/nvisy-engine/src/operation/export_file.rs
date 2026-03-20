//! File export operation.
//!
//!
//! Runs at **phase 6** alongside [`SaveContext`]. Delivers processed
//! content, optionally applying encryption and compression.
//!
//! [`SaveContext`]: crate::operation::SaveContext

//! The export pipeline applies optional post-processing steps in order:
//!
//! 1. **Encryption** — encrypt content (if config specified)
//! 2. **Compression** — compress for storage or transfer (if format specified)

use std::sync::Arc;

use nvisy_core::Result;

use crate::graph::{CompressionAlgorithm, EncryptionConfig};
use crate::operation::Operation;
use crate::operation::compression::CompressionService;
use crate::operation::context::ParallelContext;
use crate::operation::encryption::KeyProvider;

const TARGET: &str = "nvisy_engine::op::export_file";

/// Exports processed content, optionally applying encryption and
/// compression afterward.
pub struct ExportFile {
    encryption: Option<EncryptionConfig>,
    compression: Option<CompressionAlgorithm>,
    key_provider: Option<Arc<dyn KeyProvider>>,
}

impl ExportFile {
    pub fn new() -> Self {
        Self {
            encryption: None,
            compression: None,
            key_provider: None,
        }
    }

    pub fn with_encryption(mut self, config: Option<EncryptionConfig>) -> Self {
        self.encryption = config;
        self
    }

    pub fn with_compression(mut self, format: Option<CompressionAlgorithm>) -> Self {
        self.compression = format;
        self
    }

    pub fn with_key_provider(mut self, provider: Arc<dyn KeyProvider>) -> Self {
        self.key_provider = Some(provider);
        self
    }

    async fn export(&self, _data: ()) -> Result<()> {
        if let Some(ref enc_cfg) = self.encryption {
            let key_provider = self.key_provider.as_ref().ok_or_else(|| {
                nvisy_core::Error::runtime(
                    "encryption requires a KeyProvider",
                    "export_file",
                    false,
                )
            })?;
            // Validate the key exists before we need it
            key_provider.resolve(&enc_cfg.key_id)?;
            // TODO: encrypt actual content once export receives real data
            tracing::debug!(target: TARGET, key_id = %enc_cfg.key_id, "encryption configured, will apply when content export is wired");
        }

        if let Some(algorithm) = self.compression {
            tracing::debug!(target: TARGET, ?algorithm, "compressing export content");
            // Compression will apply to the actual content bytes once export
            // receives real data instead of ().
            let _ = CompressionService::new(algorithm).compress(&[])?;
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
