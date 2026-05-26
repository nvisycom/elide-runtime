//! [`BoxedImageHandler`]: type-erased wrapper over all image handler types.

use std::fmt;

use nvisy_core::Error;
use nvisy_core::content::{ContentData, ContentSource};
use nvisy_core::media::DocumentType;
use nvisy_ontology::modality::Image;

use super::ImageData;
use crate::document::LocationStream;
use crate::handler::{Handler, ImageHandler, ImageRedaction};

/// A type-erased image handler backed by a boxed trait object.
pub struct BoxedImageHandler(Box<dyn ImageHandler>);

impl BoxedImageHandler {
    /// Wrap any concrete image handler into a type-erased box.
    pub fn new<H: ImageHandler>(handler: H) -> Self {
        Self(Box::new(handler))
    }
}

impl fmt::Debug for BoxedImageHandler {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("BoxedImageHandler")
            .field(&self.0.document_type())
            .finish()
    }
}

impl Handler for BoxedImageHandler {
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
impl ImageHandler for BoxedImageHandler {
    fn locations(&self) -> LocationStream<'_, Image> {
        self.0.locations()
    }

    async fn read(&self, location: &Image) -> Option<ImageData> {
        self.0.read(location).await
    }

    async fn redact_at(
        &mut self,
        location: &Image,
        redaction: ImageRedaction,
    ) -> Result<(), Error> {
        self.0.redact_at(location, redaction).await
    }
}
