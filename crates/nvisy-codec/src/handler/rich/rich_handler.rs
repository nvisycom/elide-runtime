//! [`BoxedRichHandler`]: type-erased wrapper over all rich-document handler types.

use std::fmt;

use nvisy_core::Error;
use nvisy_core::content::{ContentData, ContentSource};
use nvisy_core::media::DocumentType;
use nvisy_ontology::entity::{ImageLocation, TextLocation};

#[cfg(feature = "pdf")]
use super::RichTextHandler;
use crate::document::SpanStream;
use crate::handler::image::ImageData;
use crate::handler::text::TextData;
use crate::handler::{Handler, ImageHandler, TextHandler};

/// A type-erased rich-document handler backed by a boxed trait object.
///
/// Since both [`TextHandler`] and [`ImageHandler`] are now directly
/// object-safe, this is a simple `Box<dyn RichHandler>` wrapper.
pub struct BoxedRichHandler(Box<dyn RichHandler>);

/// Combined text + image handler trait for rich documents (PDF, DOCX).
#[async_trait::async_trait]
pub(crate) trait RichHandler: Handler + Send + Sync {
    async fn text_spans(&self) -> SpanStream<'_, TextLocation, TextData>;
    async fn edit_text(
        &mut self,
        edits: SpanStream<'_, TextLocation, TextData>,
    ) -> Result<(), Error>;
    async fn text_value_at(&self, location: &TextLocation) -> Option<String>;

    async fn image_spans(&self) -> SpanStream<'_, ImageLocation, ImageData>;
    async fn edit_images(
        &mut self,
        edits: SpanStream<'_, ImageLocation, ImageData>,
    ) -> Result<(), Error>;
    async fn image_value_at(&self, location: &ImageLocation) -> Option<ImageData>;
}

#[cfg(feature = "pdf")]
#[async_trait::async_trait]
impl RichHandler for RichTextHandler {
    async fn text_spans(&self) -> SpanStream<'_, TextLocation, TextData> {
        TextHandler::text_spans(self).await
    }

    async fn edit_text(
        &mut self,
        edits: SpanStream<'_, TextLocation, TextData>,
    ) -> Result<(), Error> {
        TextHandler::edit_text(self, edits).await
    }

    async fn text_value_at(&self, location: &TextLocation) -> Option<String> {
        TextHandler::value_at(self, location).await
    }

    async fn image_spans(&self) -> SpanStream<'_, ImageLocation, ImageData> {
        ImageHandler::image_spans(self).await
    }

    async fn edit_images(
        &mut self,
        edits: SpanStream<'_, ImageLocation, ImageData>,
    ) -> Result<(), Error> {
        ImageHandler::edit_images(self, edits).await
    }

    async fn image_value_at(&self, location: &ImageLocation) -> Option<ImageData> {
        ImageHandler::value_at(self, location).await
    }
}

impl BoxedRichHandler {
    fn new<H: RichHandler + 'static>(handler: H) -> Self {
        Self(Box::new(handler))
    }
}

impl fmt::Debug for BoxedRichHandler {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("BoxedRichHandler")
            .field(&self.0.document_type())
            .finish()
    }
}

#[cfg(feature = "pdf")]
impl From<RichTextHandler> for BoxedRichHandler {
    fn from(h: RichTextHandler) -> Self {
        Self::new(h)
    }
}

impl Handler for BoxedRichHandler {
    fn document_type(&self) -> DocumentType {
        Handler::document_type(self.0.as_ref())
    }

    fn source(&self) -> ContentSource {
        Handler::source(self.0.as_ref())
    }

    fn encode(&self) -> Result<ContentData, Error> {
        Handler::encode(self.0.as_ref())
    }
}

#[async_trait::async_trait]
impl TextHandler for BoxedRichHandler {
    async fn text_spans(&self) -> SpanStream<'_, TextLocation, TextData> {
        self.0.text_spans().await
    }

    async fn edit_text(
        &mut self,
        edits: SpanStream<'_, TextLocation, TextData>,
    ) -> Result<(), Error> {
        self.0.edit_text(edits).await
    }

    async fn value_at(&self, location: &TextLocation) -> Option<String> {
        self.0.text_value_at(location).await
    }
}

#[async_trait::async_trait]
impl ImageHandler for BoxedRichHandler {
    async fn image_spans(&self) -> SpanStream<'_, ImageLocation, ImageData> {
        self.0.image_spans().await
    }

    async fn edit_images(
        &mut self,
        edits: SpanStream<'_, ImageLocation, ImageData>,
    ) -> Result<(), Error> {
        self.0.edit_images(edits).await
    }

    async fn value_at(&self, location: &ImageLocation) -> Option<ImageData> {
        self.0.image_value_at(location).await
    }
}
