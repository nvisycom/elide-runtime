//! Image format handlers and loaders.

mod image_data;
mod image_handler_macro;

mod jpeg_handler;
mod jpeg_loader;

mod png_handler;
mod png_loader;

pub use image_data::ImageData;
pub(crate) use image_handler_macro::impl_image_handler;

pub use png_handler::PngHandler;
pub use png_loader::{PngLoader, PngParams};

pub use jpeg_handler::JpegHandler;
pub use jpeg_loader::{JpegLoader, JpegParams};

use image::DynamicImage;
use nvisy_core::Error;
use nvisy_core::io::ContentData;

/// Decode raw bytes into a [`DynamicImage`].
///
/// Shared by all image loaders.
pub(crate) fn decode_image(content: &ContentData, origin: &str) -> Result<DynamicImage, Error> {
    let raw = content.to_bytes();
    image::load_from_memory(&raw)
        .map_err(|e| Error::validation(format!("image decode failed: {e}"), origin))
}
