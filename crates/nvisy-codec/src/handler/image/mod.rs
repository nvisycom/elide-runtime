//! Image format handlers and loaders.

use nvisy_core::Error;
use nvisy_ontology::entity::ImageLocation;

use super::Handler;
use crate::document::LocationStream;
use crate::transform::{ImageRedaction, Redactions};

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
/// Handlers expose image content as a stream of [`ImageLocation`]s
/// (cheap, identity-only), with explicit `read` calls to fetch the
/// payload for any given location, and a `redact` call that applies a
/// batch of [`ImageRedaction`]s grouped by location.
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

    /// Apply a batch of redactions grouped by [`ImageLocation`].
    async fn redact(
        &mut self,
        redactions: Redactions<ImageLocation, ImageRedaction>,
    ) -> Result<(), Error>;
}
