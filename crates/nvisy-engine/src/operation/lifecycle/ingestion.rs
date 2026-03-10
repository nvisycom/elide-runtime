//! Content ingestion operation.
//!
//! Takes raw [`ContentData`] and decodes it into a [`Document`] using
//! magic-byte detection. This is the entry point of the processing
//! pipeline: upstream stages provide raw bytes, and downstream stages
//! receive a fully parsed, type-erased document.

use nvisy_codec::Document;
use nvisy_core::Result;
use nvisy_core::content::ContentData;

use crate::operation::{Operation, ParallelContext};

const TARGET: &str = "nvisy_engine::op::ingestion";

/// Decodes raw content into a [`Document`].
///
/// Uses [`Document::decode`] to infer the format from magic bytes and
/// dispatch to the appropriate loader. Text formats (TXT, CSV, JSON)
/// lack magic-byte signatures and must be loaded explicitly via their
/// respective loaders before reaching this operation.
pub struct Ingestion;

impl Ingestion {
    #[tracing::instrument(target = TARGET, skip_all, fields(size = content.size()))]
    async fn decode(&self, content: ContentData) -> Result<Document> {
        let doc = Document::decode(&content).await?;
        tracing::debug!(target: TARGET, doc_type = %doc.document_type(), "decoded document");
        Ok(doc)
    }
}

impl Operation for Ingestion {
    type Input = ParallelContext<ContentData>;
    type Output = ParallelContext<Document>;

    async fn call(&self, input: Self::Input) -> Result<Self::Output> {
        input.parallel_map(|data| self.decode(data)).await
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
        assert!(Ingestion.call(input).await.is_err());
    }
}
