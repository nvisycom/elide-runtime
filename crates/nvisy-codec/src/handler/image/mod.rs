//! Image format handlers and loaders.

use nvisy_core::Error;

use super::Handler;
use crate::document::SpanStream;

mod image_data;
mod image_handler;
mod image_handler_macro;
mod image_span_id;

mod jpeg_handler;
mod jpeg_loader;

mod png_handler;
mod png_loader;

pub use self::image_data::ImageData;
pub use self::image_handler::BoxedImageHandler;
pub(crate) use self::image_handler_macro::impl_image_handler;
pub use self::image_span_id::ImageSpanId;
pub use self::jpeg_handler::JpegHandler;
pub use self::jpeg_loader::{JpegLoader, JpegParams};
pub use self::png_handler::PngHandler;
pub use self::png_loader::{PngLoader, PngParams};

/// Capability trait for handlers that expose image content.
///
/// All image handlers use [`ImageSpanId`] as their span identifier,
/// making this trait directly object-safe without a `Dyn*` wrapper.
#[async_trait::async_trait]
pub trait ImageHandler: Handler {
    /// Return image content as an async stream of [`Span`](crate::document::Span)s.
    ///
    /// Each span carries an [`ImageSpanId`] and [`ImageData`] payload.
    async fn image_spans(&self) -> SpanStream<'_, ImageSpanId, ImageData>;

    /// Apply image edits from an async stream back to the handler.
    ///
    /// The stream items must use the same [`ImageSpanId`] returned by
    /// [`image_spans`](Self::image_spans).
    async fn edit_images(
        &mut self,
        edits: SpanStream<'_, ImageSpanId, ImageData>,
    ) -> Result<(), Error>;
}
