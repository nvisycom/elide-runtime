//! [`BoxedImageHandler`]: type-erased wrapper over all image handler types.

use nvisy_core::Error;
use nvisy_core::content::{ContentData, ContentSource};
use nvisy_core::media::DocumentType;

use super::{ImageData, ImageSpanId, JpegHandler, PngHandler};
use crate::document::SpanStream;
use crate::handler::{Handler, ImageHandler};

/// A type-erased image handler backed by a boxed trait object.
///
/// Since [`ImageHandler`] uses a concrete [`ImageSpanId`] (no associated
/// type), the trait is directly object-safe and can be boxed without a
/// private `Dyn*` indirection layer.
pub struct BoxedImageHandler(Box<dyn ImageHandler>);

impl BoxedImageHandler {
    /// Wrap any concrete image handler into a type-erased box.
    pub fn new<H: ImageHandler>(handler: H) -> Self {
        Self(Box::new(handler))
    }
}

impl std::fmt::Debug for BoxedImageHandler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
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
    async fn image_spans(&self) -> SpanStream<'_, ImageSpanId, ImageData> {
        self.0.image_spans().await
    }

    async fn edit_images(
        &mut self,
        edits: SpanStream<'_, ImageSpanId, ImageData>,
    ) -> Result<(), Error> {
        self.0.edit_images(edits).await
    }
}

#[cfg(test)]
mod tests {
    use futures::StreamExt;

    use super::*;

    fn make_png() -> PngHandler {
        let img = image::DynamicImage::new_rgb8(1, 1);
        PngHandler::new(img)
    }

    fn make_jpeg() -> JpegHandler {
        let img = image::DynamicImage::new_rgb8(1, 1);
        JpegHandler::new(img)
    }

    #[test]
    fn png_variant_document_type() {
        let h = BoxedImageHandler::from(make_png());
        assert_eq!(
            h.document_type(),
            DocumentType::Image(nvisy_core::media::ImageFormat::Png),
        );
    }

    #[test]
    fn jpeg_variant_document_type() {
        let h = BoxedImageHandler::from(make_jpeg());
        assert_eq!(
            h.document_type(),
            DocumentType::Image(nvisy_core::media::ImageFormat::Jpeg),
        );
    }

    #[tokio::test]
    async fn view_spans_returns_image() {
        let h = BoxedImageHandler::from(make_png());
        let spans: Vec<_> = h.image_spans().await.collect().await;
        assert_eq!(spans.len(), 1);
    }
}
