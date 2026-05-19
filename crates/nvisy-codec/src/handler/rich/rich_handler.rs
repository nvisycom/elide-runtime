//! [`BoxedRichHandler`]: type-erased wrapper over all rich-document handler types.

use std::fmt;

use nvisy_core::Error;
use nvisy_core::content::{ContentData, ContentSource};
use nvisy_core::media::DocumentType;
use nvisy_ontology::entity::{ImageLocation, TextLocation};

#[cfg(feature = "pdf")]
use super::RichTextHandler;
use crate::document::LocationStream;
use crate::handler::image::ImageData;
use crate::handler::text::TextData;
use crate::handler::{Handler, ImageHandler, TextHandler};
use crate::transform::{ImageRedaction, TextRedaction};

/// A type-erased rich-document handler backed by a boxed trait object.
pub struct BoxedRichHandler(Box<dyn RichHandler>);

/// Combined text + image handler trait for rich documents (PDF, DOCX).
#[async_trait::async_trait]
pub(crate) trait RichHandler: Handler + Send + Sync {
    fn text_locations(&self) -> LocationStream<'_, TextLocation>;
    async fn read_text(&self, location: &TextLocation) -> Option<TextData>;
    async fn redact_text_at(
        &mut self,
        location: &TextLocation,
        redaction: TextRedaction,
    ) -> Result<(), Error>;

    fn image_locations(&self) -> LocationStream<'_, ImageLocation>;
    async fn read_image(&self, location: &ImageLocation) -> Option<ImageData>;
    async fn redact_image_at(
        &mut self,
        location: &ImageLocation,
        redaction: ImageRedaction,
    ) -> Result<(), Error>;
}

#[cfg(feature = "pdf")]
#[async_trait::async_trait]
impl RichHandler for RichTextHandler {
    fn text_locations(&self) -> LocationStream<'_, TextLocation> {
        TextHandler::locations(self)
    }

    async fn read_text(&self, location: &TextLocation) -> Option<TextData> {
        TextHandler::read(self, location).await
    }

    async fn redact_text_at(
        &mut self,
        location: &TextLocation,
        redaction: TextRedaction,
    ) -> Result<(), Error> {
        TextHandler::redact_at(self, location, redaction).await
    }

    fn image_locations(&self) -> LocationStream<'_, ImageLocation> {
        ImageHandler::locations(self)
    }

    async fn read_image(&self, location: &ImageLocation) -> Option<ImageData> {
        ImageHandler::read(self, location).await
    }

    async fn redact_image_at(
        &mut self,
        location: &ImageLocation,
        redaction: ImageRedaction,
    ) -> Result<(), Error> {
        ImageHandler::redact_at(self, location, redaction).await
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
    fn locations(&self) -> LocationStream<'_, TextLocation> {
        self.0.text_locations()
    }

    async fn read(&self, location: &TextLocation) -> Option<TextData> {
        self.0.read_text(location).await
    }

    async fn redact_at(
        &mut self,
        location: &TextLocation,
        redaction: TextRedaction,
    ) -> Result<(), Error> {
        self.0.redact_text_at(location, redaction).await
    }
}

#[async_trait::async_trait]
impl ImageHandler for BoxedRichHandler {
    fn locations(&self) -> LocationStream<'_, ImageLocation> {
        self.0.image_locations()
    }

    async fn read(&self, location: &ImageLocation) -> Option<ImageData> {
        self.0.read_image(location).await
    }

    async fn redact_at(
        &mut self,
        location: &ImageLocation,
        redaction: ImageRedaction,
    ) -> Result<(), Error> {
        self.0.redact_image_at(location, redaction).await
    }
}
