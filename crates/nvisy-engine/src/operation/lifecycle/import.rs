//! Content import: the first stage of the redaction pipeline.
//!
//! [`Import`] accepts raw [`ContentData`] (bytes + metadata), decodes it
//! into a typed [`Document`], and wraps the result in a
//! [`DocumentEnvelope`] — the per-document state object that travels
//! through every subsequent pipeline stage.
//!
//! ```text
//! ContentData (raw bytes + optional MIME)
//!   ↓ Import::call
//! DocumentEnvelope { document, entities: [], audit }
//! ```
//!
//! ## Format detection
//!
//! [`Document::decode`] resolves the format through three strategies
//! evaluated by [`ContentData::infer_document_type`]:
//!
//! 1. **Caller-supplied MIME** — set via
//!    [`ContentData::with_content_type`]. This is the only way to
//!    reach text-based formats (TXT, Log, CSV, JSON, HTML) that have
//!    no magic-byte signature.
//!
//! 2. **Magic-byte detection** — binary formats (PNG, JPEG, WAV, MP3,
//!    PDF, DOCX, XLSX) are identified by their file signatures,
//!    eagerly computed on [`ContentData`] construction.
//!
//! 3. **Filename extension** — set via
//!    [`ContentData::with_filename`]. Refines ambiguous results
//!    (e.g. distinguishes `.log` from `.txt` when MIME is
//!    `text/plain`).
//!
//! All three are always evaluated. If a supplied MIME and magic bytes
//! disagree, a warning is logged and the supplied MIME takes precedence.
//!
//! [`ContentData`]: nvisy_core::content::ContentData
//! [`ContentData::with_content_type`]: nvisy_core::content::ContentData::with_content_type
//! [`ContentData::with_filename`]: nvisy_core::content::ContentData::with_filename
//! [`ContentData::infer_document_type`]: nvisy_core::content::ContentData::infer_document_type

use nvisy_codec::Document;
use nvisy_core::Result;
use nvisy_core::content::ContentData;

use crate::operation::{DocumentEnvelope, Operation, ParallelContext};

const TARGET: &str = "nvisy_engine::op::import";

/// Decodes raw content into a [`DocumentEnvelope`].
///
/// This is the entry point of the processing pipeline. Upstream stages
/// provide raw bytes via [`ContentData`], and this operation produces a
/// fully parsed [`DocumentEnvelope`] with an empty entity set and a
/// freshly initialised [`Audit`] trail anchored to the document's
/// [`ContentSource`].
///
/// [`Audit`]: crate::provenance::Audit
/// [`ContentSource`]: nvisy_core::content::ContentSource
/// [`ContentData`]: nvisy_core::content::ContentData
pub struct Import;

impl Import {
    /// Decode raw bytes into a [`Document`] and wrap it in a fresh envelope.
    #[tracing::instrument(target = TARGET, skip_all, fields(size = content.size()))]
    async fn import(&self, content: ContentData) -> Result<DocumentEnvelope> {
        let doc = Document::decode(&content).await?;
        tracing::debug!(target: TARGET, doc_type = %doc.document_type(), "decoded document");
        Ok(DocumentEnvelope::new(doc))
    }
}

impl Operation for Import {
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
        let shared = SharedContext::new(uuid::Uuid::new_v4(), uuid::Uuid::new_v4());
        let content = ContentData::from("plain text has no magic bytes");
        let input = ParallelContext::new(content, shared);
        assert!(Import.call(input).await.is_err());
    }
}
