//! Unified document representation.

mod any;
pub(crate) mod span;
pub(crate) mod stream;

pub use any::AnyDocument;
use derive_more::{Deref, DerefMut};
use futures::StreamExt;
use nvisy_core::Error;
use nvisy_core::fs::DocumentType;
use nvisy_core::io::ContentData;
use nvisy_core::path::ContentSource;
pub use span::{Span, SpanEdit};
pub use stream::{SpanEditStream, SpanStream};

use crate::handler::{
    AudioData, AudioHandler, Handler, ImageData, ImageHandler, TextData, TextHandler,
};

/// A unified representation of any content that can be handled by the pipeline.
///
/// `Document` is generic over `H`, a [`Handler`] that holds the loaded data
/// and provides methods to read and manipulate it.
#[derive(Debug, Deref, DerefMut)]
pub struct Document<H: Handler> {
    /// Content source identity and lineage.
    pub source: ContentSource,

    /// Format handler (holds the loaded data).
    #[deref]
    #[deref_mut]
    handler: H,
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

    /// Encode the handler content and set this document's source as the parent.
    pub fn encode(&self) -> Result<ContentData, Error> {
        let mut content = self.handler.encode()?;
        content
            .content_source
            .set_parent_id(Some(self.source.as_uuid()));
        Ok(content)
    }

    /// Set this document's parent to the given content source.
    pub fn with_parent(mut self, content: &ContentData) -> Self {
        self.source
            .set_parent_id(Some(content.content_source.as_uuid()));
        self
    }
}

// Conditional impls for capability traits.

impl<H: TextHandler> Document<H> {
    /// View text spans with the document's content source injected.
    pub async fn text_spans(&self) -> SpanStream<'_, H::TextId, TextData> {
        let source = self.source;
        let inner = self.handler.text_spans().await;
        SpanStream::new(inner.map(move |mut span| {
            span.source = source;
            span
        }))
    }

    /// Apply text edits from an async stream back to the handler.
    pub async fn edit_text(
        &mut self,
        edits: SpanEditStream<'_, H::TextId, TextData>,
    ) -> Result<(), Error> {
        self.handler.edit_text(edits).await
    }
}

impl<H: ImageHandler> Document<H> {
    /// View image spans with the document's content source injected.
    pub async fn image_spans(&self) -> SpanStream<'_, H::ImageId, ImageData> {
        let source = self.source;
        let inner = self.handler.image_spans().await;
        SpanStream::new(inner.map(move |mut span| {
            span.source = source;
            span
        }))
    }

    /// Apply image edits from an async stream back to the handler.
    pub async fn edit_images(
        &mut self,
        edits: SpanEditStream<'_, H::ImageId, ImageData>,
    ) -> Result<(), Error> {
        self.handler.edit_images(edits).await
    }
}

impl<H: AudioHandler> Document<H> {
    /// View audio spans with the document's content source injected.
    pub async fn audio_spans(&self) -> SpanStream<'_, H::AudioId, AudioData> {
        let source = self.source;
        let inner = self.handler.audio_spans().await;
        SpanStream::new(inner.map(move |mut span| {
            span.source = source;
            span
        }))
    }

    /// Apply audio edits from an async stream back to the handler.
    pub async fn edit_audio(
        &mut self,
        edits: SpanEditStream<'_, H::AudioId, AudioData>,
    ) -> Result<(), Error> {
        self.handler.edit_audio(edits).await
    }
}

#[cfg(test)]
mod tests {
    use futures::StreamExt;

    use super::*;
    use crate::handler::{TxtHandler, TxtSpan};

    #[tokio::test]
    async fn text_spans_injects_source() {
        let handler = TxtHandler::new(vec!["line".into()], false);
        let doc = Document::new(handler);
        let source = doc.source;
        let spans: Vec<_> = doc.text_spans().await.collect().await;
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].source, source);
        assert_eq!(spans[0].data, "line");
    }

    #[tokio::test]
    async fn edit_text_delegates_to_handler() -> Result<(), Error> {
        let handler = TxtHandler::new(vec!["original".into()], false);
        let mut doc = Document::new(handler);
        doc.edit_text(SpanEditStream::new(futures::stream::iter(vec![
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
        assert_eq!(
            doc.document_type(),
            DocumentType::Text(nvisy_core::fs::TextFormat::Txt),
        );
    }

    #[test]
    fn encode_sets_parent_id() {
        let handler = TxtHandler::new(vec!["hello".into()], false);
        let doc = Document::new(handler);
        let source = doc.source;
        let content = doc.encode().unwrap();
        assert_eq!(content.content_source.parent_id(), Some(source.as_uuid()),);
    }

    #[test]
    fn with_parent_sets_lineage() {
        let handler = TxtHandler::new(vec![], false);
        let doc = Document::new(handler);
        let content = ContentData::new(ContentSource::new(), bytes::Bytes::from_static(b"parent"));
        let doc = doc.with_parent(&content);
        assert_eq!(
            doc.source.parent_id(),
            Some(content.content_source.as_uuid()),
        );
    }
}
