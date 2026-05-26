//! [`RichHandler`] trait + [`BoxedRichHandler`] type-erased wrapper.

use std::fmt;

use nvisy_core::Error;
use nvisy_core::content::{ContentData, ContentSource};
use nvisy_core::media::DocumentType;
use nvisy_ontology::modality::{Image, Text};

use crate::document::LocationStream;
use crate::handler::image::ImageData;
use crate::handler::text::TextData;
use crate::handler::{Handler, ImageHandler, ImageRedaction, TextHandler, TextRedaction};

/// A type-erased rich-document handler backed by a boxed trait object.
pub struct BoxedRichHandler(Box<dyn RichHandler>);

/// Combined text + image handler trait for rich documents (PDF, DOCX, …).
#[async_trait::async_trait]
pub trait RichHandler: Handler + Send + Sync {
    /// Stream the text locations exposed by this rich document.
    fn text_locations(&self) -> LocationStream<'_, Text>;
    /// Read the text content at the given location.
    async fn read_text(&self, location: &Text) -> Option<TextData>;
    /// Apply a single text redaction at the given location.
    async fn redact_text_at(
        &mut self,
        location: &Text,
        redaction: TextRedaction,
    ) -> Result<(), Error>;

    /// Stream the image locations exposed by this rich document.
    fn image_locations(&self) -> LocationStream<'_, Image>;
    /// Read the image content at the given location.
    async fn read_image(&self, location: &Image) -> Option<ImageData>;
    /// Apply a single image redaction at the given location.
    async fn redact_image_at(
        &mut self,
        location: &Image,
        redaction: ImageRedaction,
    ) -> Result<(), Error>;
}

impl BoxedRichHandler {
    /// Wrap any concrete rich-document handler.
    pub fn new<H: RichHandler + 'static>(handler: H) -> Self {
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
    fn locations(&self) -> LocationStream<'_, Text> {
        self.0.text_locations()
    }

    async fn read(&self, location: &Text) -> Option<TextData> {
        self.0.read_text(location).await
    }

    async fn redact_at(
        &mut self,
        location: &Text,
        redaction: TextRedaction,
    ) -> Result<(), Error> {
        self.0.redact_text_at(location, redaction).await
    }
}

#[async_trait::async_trait]
impl ImageHandler for BoxedRichHandler {
    fn locations(&self) -> LocationStream<'_, Image> {
        self.0.image_locations()
    }

    async fn read(&self, location: &Image) -> Option<ImageData> {
        self.0.read_image(location).await
    }

    async fn redact_at(
        &mut self,
        location: &Image,
        redaction: ImageRedaction,
    ) -> Result<(), Error> {
        self.0.redact_image_at(location, redaction).await
    }
}
