//! File import operation.
//!
//!
//! Runs at **phase 0** alongside [`LoadContext`]. Decodes raw content
//! into a [`DocumentEnvelope`], optionally applying decompression and
//! decryption.
//!
//! [`LoadContext`]: crate::operation::LoadContext

//! The import pipeline applies optional pre-processing steps in order:
//!
//! 1. **Decompression** — decompress raw bytes (if format specified)
//! 2. **Decryption** — decrypt content (if format specified)
//! 3. **Decode** — detect format and decode into a typed [`Document`]
//!
//! [`Document`]: nvisy_codec::Document

use nvisy_codec::Document;
use nvisy_core::Result;
use nvisy_core::content::ContentData;

use crate::graph::{CompressionFormat, EncryptionFormat};
use crate::operation::context::ParallelContext;
use crate::operation::{DocumentEnvelope, Operation};

const TARGET: &str = "nvisy_engine::op::import_file";

/// Decodes raw content into a [`DocumentEnvelope`], optionally applying
/// decompression and decryption beforehand.
pub struct ImportFile {
    decompression: Option<CompressionFormat>,
    decryption: Option<EncryptionFormat>,
}

impl ImportFile {
    pub fn new() -> Self {
        Self {
            decompression: None,
            decryption: None,
        }
    }

    pub fn with_decompression(mut self, format: Option<CompressionFormat>) -> Self {
        self.decompression = format;
        self
    }

    pub fn with_decryption(mut self, format: Option<EncryptionFormat>) -> Self {
        self.decryption = format;
        self
    }

    async fn import(&self, content: ContentData) -> Result<DocumentEnvelope> {
        let data = content;

        if let Some(format) = self.decompression {
            return Err(nvisy_core::Error::runtime(
                format!("import decompression ({format:?}) is not yet implemented"),
                "import_file",
                false,
            ));
        }
        if let Some(format) = self.decryption {
            return Err(nvisy_core::Error::runtime(
                format!("import decryption ({format:?}) is not yet implemented"),
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
    use crate::operation::context::SharedContext;

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
