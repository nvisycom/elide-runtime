//! Image format handlers and loaders.

use nvisy_core::Error;

use super::Handler;
use crate::document::SpanStream;

mod image_data;
mod image_handler;
mod image_handler_macro;

mod jpeg_handler;
mod jpeg_loader;

mod png_handler;
mod png_loader;

pub use image_data::ImageData;
pub use image_handler::BoxedImageHandler;
pub(crate) use image_handler_macro::impl_image_handler;
pub use jpeg_handler::JpegHandler;
pub use jpeg_loader::{JpegLoader, JpegParams};
pub use png_handler::PngHandler;
pub use png_loader::{PngLoader, PngParams};

/// Identifier for an image span within a single-image handler.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ImageSpanId;

/// Capability trait for handlers that expose image content.
///
/// Handlers implementing this trait can yield image spans and accept
/// image edits. The associated [`ImageId`](Self::ImageId) type allows
/// different handlers to use different addressing schemes (e.g.
/// [`ImageSpanId`] for standalone images, `RichImageSpan` for images
/// embedded in multi-page documents).
#[async_trait::async_trait]
pub trait ImageHandler: Handler {
    /// Strongly-typed identifier for an image span within this handler.
    ///
    /// Standalone image handlers use [`ImageSpanId`] (a unit type),
    /// while rich-document handlers may use a page-aware identifier.
    type ImageId: Send + Sync + Clone + 'static;

    /// Return image content as an async stream of [`Span`](crate::document::Span)s.
    ///
    /// Each span carries an [`ImageId`](Self::ImageId) and [`ImageData`] payload.
    async fn image_spans(&self) -> SpanStream<'_, Self::ImageId, ImageData>;

    /// Apply image edits from an async stream back to the handler.
    ///
    /// The stream items must use the same [`ImageId`](Self::ImageId)
    /// returned by [`image_spans`](Self::image_spans).
    async fn edit_images(
        &mut self,
        edits: SpanStream<'_, Self::ImageId, ImageData>,
    ) -> Result<(), Error>;
}

use image::DynamicImage;
use nvisy_core::io::ContentData;

/// Decode raw bytes into a [`DynamicImage`].
///
/// Shared by all image loaders.
pub(crate) fn decode_image(content: &ContentData, origin: &str) -> Result<DynamicImage, Error> {
    let raw = content.to_bytes();
    image::load_from_memory(&raw)
        .map_err(|e| Error::validation(format!("image decode failed: {e}"), origin))
}
