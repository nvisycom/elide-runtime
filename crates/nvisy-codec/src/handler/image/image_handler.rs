//! [`AnyImage`]: type-erased wrapper over all image handler types.

use derive_more::From;
use futures::StreamExt;
use nvisy_core::Error;
use nvisy_core::fs::DocumentType;
use nvisy_core::io::ContentData;

use super::{ImageData, JpegHandler, PngHandler};
use crate::document::{SpanEditStream, SpanStream};
use crate::handler::{Handler, ImageHandler};

/// A type-erased image handler that can hold any supported image format.
///
/// Since all image handlers share `ImageId = ()`, this enum can
/// implement [`Handler`] + [`ImageHandler`] directly.
#[derive(Debug, From)]
pub enum AnyImage {
    Png(PngHandler),
    Jpeg(JpegHandler),
}

impl AnyImage {
    /// Try to get the inner [`PngHandler`] by reference.
    pub fn as_png(&self) -> Option<&PngHandler> {
        if let Self::Png(h) = self {
            Some(h)
        } else {
            None
        }
    }

    /// Consume and return the inner [`PngHandler`].
    pub fn into_png(self) -> Option<PngHandler> {
        if let Self::Png(h) = self {
            Some(h)
        } else {
            None
        }
    }

    /// Try to get the inner [`JpegHandler`] by reference.
    pub fn as_jpeg(&self) -> Option<&JpegHandler> {
        if let Self::Jpeg(h) = self {
            Some(h)
        } else {
            None
        }
    }

    /// Consume and return the inner [`JpegHandler`].
    pub fn into_jpeg(self) -> Option<JpegHandler> {
        if let Self::Jpeg(h) = self {
            Some(h)
        } else {
            None
        }
    }
}

impl Handler for AnyImage {
    fn document_type(&self) -> DocumentType {
        match self {
            Self::Png(h) => h.document_type(),
            Self::Jpeg(h) => h.document_type(),
        }
    }

    fn encode(&self) -> Result<ContentData, Error> {
        match self {
            Self::Png(h) => h.encode(),
            Self::Jpeg(h) => h.encode(),
        }
    }
}

#[async_trait::async_trait]
impl ImageHandler for AnyImage {
    type ImageId = ();

    async fn image_spans(&self) -> SpanStream<'_, (), ImageData> {
        match self {
            Self::Png(h) => h.image_spans().await,
            Self::Jpeg(h) => h.image_spans().await,
        }
    }

    async fn edit_images(&mut self, edits: SpanEditStream<'_, (), ImageData>) -> Result<(), Error> {
        let edits: Vec<_> = edits.collect().await;
        let stream = SpanEditStream::new(futures::stream::iter(edits));
        match self {
            Self::Png(h) => h.edit_images(stream).await,
            Self::Jpeg(h) => h.edit_images(stream).await,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::handler::ImageHandler;

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
        let h = AnyImage::Png(make_png());
        assert_eq!(
            h.document_type(),
            DocumentType::Image(nvisy_core::fs::ImageFormat::Png),
        );
    }

    #[test]
    fn jpeg_variant_document_type() {
        let h = AnyImage::Jpeg(make_jpeg());
        assert_eq!(
            h.document_type(),
            DocumentType::Image(nvisy_core::fs::ImageFormat::Jpeg),
        );
    }

    #[tokio::test]
    async fn view_spans_returns_image() {
        let h = AnyImage::Png(make_png());
        let spans: Vec<_> = h.image_spans().await.collect().await;
        assert_eq!(spans.len(), 1);
    }

    #[test]
    fn from_conversions() {
        let png: AnyImage = make_png().into();
        assert!(png.as_png().is_some());
        let jpeg: AnyImage = make_jpeg().into();
        assert!(jpeg.as_jpeg().is_some());
    }

    #[test]
    fn encode_delegates() {
        let h = AnyImage::Png(make_png());
        assert!(h.encode().is_ok());
    }
}
