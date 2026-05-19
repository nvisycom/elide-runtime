//! [`BoxedImageHandler`]: type-erased wrapper over all image handler types.

use std::fmt;

use nvisy_core::Error;
use nvisy_core::content::{ContentData, ContentSource};
use nvisy_core::media::DocumentType;
use nvisy_ontology::entity::ImageLocation;

use super::{ImageData, JpegHandler, PngHandler, TiffHandler};
use crate::document::LocationStream;
use crate::handler::{Handler, ImageHandler};
use crate::transform::ImageRedaction;

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

impl From<PngHandler> for BoxedImageHandler {
    fn from(h: PngHandler) -> Self {
        Self::new(h)
    }
}

impl From<JpegHandler> for BoxedImageHandler {
    fn from(h: JpegHandler) -> Self {
        Self::new(h)
    }
}

impl From<TiffHandler> for BoxedImageHandler {
    fn from(h: TiffHandler) -> Self {
        Self::new(h)
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
    fn locations(&self) -> LocationStream<'_, ImageLocation> {
        self.0.locations()
    }

    async fn read(&self, location: &ImageLocation) -> Option<ImageData> {
        self.0.read(location).await
    }

    async fn redact_at(
        &mut self,
        location: &ImageLocation,
        redaction: ImageRedaction,
    ) -> Result<(), Error> {
        self.0.redact_at(location, redaction).await
    }
}
