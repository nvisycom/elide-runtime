//! Image format handlers and loaders.

use nvisy_core::Error;
use nvisy_ontology::entity::ImageLocation;

use super::Handler;
use crate::document::SpanStream;

mod image_data;
mod image_handler;
mod image_handler_macro;

mod jpeg_handler;
mod jpeg_loader;

mod png_handler;
mod png_loader;

mod tiff_handler;
mod tiff_loader;

pub use self::image_data::ImageData;
pub use self::image_handler::BoxedImageHandler;
pub(crate) use self::image_handler_macro::impl_image_handler;
pub use self::jpeg_handler::JpegHandler;
pub use self::jpeg_loader::{JpegLoader, JpegParams};
pub use self::png_handler::PngHandler;
pub use self::png_loader::{PngLoader, PngParams};
pub use self::tiff_handler::TiffHandler;
pub use self::tiff_loader::{TiffLoader, TiffParams};

/// Capability trait for handlers that expose image content.
///
/// All image handlers use [`ImageLocation`] as their span identifier.
#[async_trait::async_trait]
pub trait ImageHandler: Handler {
    /// Return image content as an async stream of [`Span`](crate::document::Span)s.
    ///
    /// Each span carries an [`ImageLocation`] and [`ImageData`] payload.
    async fn image_spans(&self) -> SpanStream<'_, ImageLocation, ImageData>;

    /// Apply image edits from an async stream back to the handler.
    async fn edit_images(
        &mut self,
        edits: SpanStream<'_, ImageLocation, ImageData>,
    ) -> Result<(), Error>;

    /// Extract the image data at the given location (crop the bounding box).
    ///
    /// Returns `None` if the location is out of bounds.
    async fn value_at(&self, location: &ImageLocation) -> Option<ImageData>;
}
