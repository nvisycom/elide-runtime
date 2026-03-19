//! File import: decode raw content into a [`DocumentEnvelope`].
//!
//! The import pipeline applies optional pre-processing steps in order:
//!
//! 1. **Decompression** — decompress raw bytes (if `decompression` is set)
//! 2. **Decryption** — decrypt content (if `decryption` is set)
//! 3. **Decode** — detect format and decode into a typed [`Document`]
//!
//! [`Document`]: nvisy_codec::Document

use nvisy_codec::Document;
use nvisy_core::Result;
use nvisy_core::content::ContentData;

use crate::operation::{DocumentEnvelope, Operation, ParallelContext};

const TARGET: &str = "nvisy_engine::op::import_file";

/// Decodes raw content into a [`DocumentEnvelope`], optionally applying
/// decompression and decryption beforehand.
pub struct ImportFile {
    decompression: bool,
    decryption: bool,
}

impl ImportFile {
    /// Create a new import operation with default settings (no pre-processing).
    pub fn new() -> Self {
        Self {
            decompression: false,
            decryption: false,
        }
    }

    /// Enable decompression before decoding.
    pub fn with_decompression(mut self, enabled: bool) -> Self {
        self.decompression = enabled;
        self
    }

    /// Enable decryption before decoding.
    pub fn with_decryption(mut self, enabled: bool) -> Self {
        self.decryption = enabled;
        self
    }

    async fn import(&self, content: ContentData) -> Result<DocumentEnvelope> {
        let data = content;

        if self.decompression {
            return Err(nvisy_core::Error::runtime(
                "import decompression is not yet implemented",
                "import_file",
                false,
            ));
        }
        if self.decryption {
            return Err(nvisy_core::Error::runtime(
                "import decryption is not yet implemented",
                "import_file",
                false,
            ));
        }

        let doc = Document::decode(&data).await?;
        tracing::debug!(target: TARGET, doc_type = %doc.document_type(), "decoded document");
        Ok(DocumentEnvelope::new(doc))
    }
}

impl Default for ImportFile {
    fn default() -> Self {
        Self::new()
    }
}

impl Operation for ImportFile {
    type Input = ParallelContext<ContentData>;
    type Output = ParallelContext<DocumentEnvelope>;

    async fn call(&self, input: Self::Input) -> Result<Self::Output> {
        input.parallel_map(|data| self.import(data)).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::operation::SharedContext;

    #[tokio::test]
    async fn unknown_format_errors() {
        let dir = tempfile::tempdir().unwrap();
        let registry = nvisy_registry::Registry::open(dir.path()).unwrap();
        let shared = SharedContext::new(uuid::Uuid::new_v4(), uuid::Uuid::new_v4(), registry);
        let content = ContentData::from("plain text has no magic bytes");
        let input = ParallelContext::new(content, shared);
        assert!(ImportFile::new().call(input).await.is_err());
    }
}
