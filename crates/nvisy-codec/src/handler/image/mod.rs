//! Image format handlers and loaders.

use nvisy_core::Error;
use nvisy_ontology::entity::ImageLocation;

use super::Handler;
use crate::document::LocationStream;
use crate::transform::ImageRedaction;

mod apply;
mod image_data;
mod image_handler;
mod image_handler_macro;
mod ops;

mod jpeg_handler;
mod jpeg_loader;

mod png_handler;
mod png_loader;

mod tiff_handler;
mod tiff_loader;

pub(crate) use self::apply::apply_image_redaction;
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
/// Handlers implement three narrow operations:
/// - [`locations`]: cheap, identity-only stream of [`ImageLocation`]s.
/// - [`read`]: fetch the payload at a given location (cropped to
///   the location's bounding box).
/// - [`redact_at`]: apply a single redaction to a single location.
///
/// Batched redaction lives on the blanket-impl [`ImageTransform::redact`].
///
/// [`locations`]: ImageHandler::locations
/// [`read`]: ImageHandler::read
/// [`redact_at`]: ImageHandler::redact_at
/// [`ImageTransform::redact`]: crate::transform::ImageTransform::redact
#[async_trait::async_trait]
pub trait ImageHandler: Handler {
    /// Async stream of [`ImageLocation`]s for this document, each
    /// tagged with the handler's [`ContentSource`].
    ///
    /// [`ContentSource`]: nvisy_core::content::ContentSource
    fn locations(&self) -> LocationStream<'_, ImageLocation>;

    /// Read the image data at the given location (crop the bounding box).
    ///
    /// Returns `None` if the location is out of bounds.
    async fn read(&self, location: &ImageLocation) -> Option<ImageData>;

    /// Apply a single redaction at the bounding box identified by
    /// `location`, mutating in place.
    async fn redact_at(
        &mut self,
        location: &ImageLocation,
        redaction: ImageRedaction,
    ) -> Result<(), Error>;
}
