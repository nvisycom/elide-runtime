//! Unified document representation.

mod any;
mod loader;

pub use any::AnyDocument;
pub use loader::UniversalLoader;

// Re-export stream types for convenience (canonical home is `crate::stream`).
#[doc(inline)]
pub use crate::stream::{SpanEditStream, SpanStream};

use std::ops::{Deref, DerefMut};

use futures::StreamExt;

use nvisy_core::Error;
use nvisy_core::io::ContentData;
use nvisy_core::path::ContentSource;
use nvisy_core::fs::DocumentType;

use crate::handler::Handler;

/// A unified representation of any content that can be handled by the pipeline.
///
/// `Document` is generic over `H`, a [`Handler`] that holds the loaded data
/// and provides methods to read and manipulate it.
#[derive(Debug)]
pub struct Document<H: Handler> {
    /// Content source identity and lineage.
    pub source: ContentSource,

    /// Format handler (holds the loaded data).
    handler: H,
}

impl<H: Handler + Clone> Clone for Document<H> {
    fn clone(&self) -> Self {
        Self {
            source: self.source,
            handler: self.handler.clone(),
        }
    }
}

impl<H: Handler> Deref for Document<H> {
    type Target = H;

    fn deref(&self) -> &H {
        &self.handler
    }
}

impl<H: Handler> DerefMut for Document<H> {
    fn deref_mut(&mut self) -> &mut H {
        &mut self.handler
    }
}

impl<H: Handler> Document<H> {
    /// Create a new document with the given handler.
    pub fn new(handler: H) -> Self {
        Self {
            source: ContentSource::new(),
            handler,
        }
    }

    /// Get a reference to the format handler.
    pub fn handler(&self) -> &H {
        &self.handler
    }

    /// Get a mutable reference to the format handler.
    pub fn handler_mut(&mut self) -> &mut H {
        &mut self.handler
    }

    /// The document type of the loaded content.
    pub fn document_type(&self) -> DocumentType {
        self.handler.document_type()
    }

    /// Map the handler into a different type, preserving the source.
    pub fn map_handler<H2: Handler>(self, f: impl FnOnce(H) -> H2) -> Document<H2> {
        Document {
            source: self.source,
            handler: f(self.handler),
        }
    }

    /// Set this document's parent to the given content source.
    pub fn with_parent(mut self, content: &ContentData) -> Self {
        self.source.set_parent_id(Some(content.content_source.as_uuid()));
        self
    }

    /// View spans with the document's content source injected.
    pub async fn view_spans(&self) -> SpanStream<'_, H::SpanId, H::SpanData> {
        let source = self.source;
        let inner = self.handler.view_spans().await;
        SpanStream::new(inner.map(move |mut span| {
            span.source = source;
            span
        }))
    }

    /// Apply edits from an async stream back to the handler.
    pub async fn edit_spans(
        &mut self,
        edits: SpanEditStream<'_, H::SpanId, H::SpanData>,
    ) -> Result<(), Error> {
        self.handler.edit_spans(edits).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::handler::{SpanEdit, TxtHandler, TxtSpan};
    use futures::StreamExt;

    #[tokio::test]
    async fn view_spans_injects_source() {
        let handler = TxtHandler::new(vec!["line".into()], false);
        let doc = Document::new(handler);
        let source = doc.source;
        let spans: Vec<_> = doc.view_spans().await.collect().await;
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].source, source);
        assert_eq!(spans[0].data, "line");
    }

    #[tokio::test]
    async fn edit_spans_delegates_to_handler() -> Result<(), Error> {
        let handler = TxtHandler::new(vec!["original".into()], false);
        let mut doc = Document::new(handler);
        doc.edit_spans(SpanEditStream::new(futures::stream::iter(vec![
            SpanEdit::new(TxtSpan(0), "edited".into()),
        ])))
        .await?;
        assert_eq!(doc.handler().lines(), &["edited"]);
        Ok(())
    }

    #[test]
    fn document_type_delegates() {
        let handler = TxtHandler::new(vec![], false);
        let doc = Document::new(handler);
        assert_eq!(doc.document_type(), DocumentType::Txt);
    }

    #[test]
    fn with_parent_sets_lineage() {
        let handler = TxtHandler::new(vec![], false);
        let doc = Document::new(handler);
        let content = ContentData::new(
            ContentSource::new(),
            bytes::Bytes::from_static(b"parent"),
        );
        let doc = doc.with_parent(&content);
        assert_eq!(
            doc.source.parent_id(),
            Some(content.content_source.as_uuid()),
        );
    }
}
