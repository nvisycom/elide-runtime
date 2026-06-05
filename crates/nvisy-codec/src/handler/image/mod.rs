//! Image modality: `impl Codable for Image` plus concrete image
//! format implementations (PNG, JPEG, TIFF) and pixel-decode helpers.
//!
//! Replacements written during [`IndexedHandle::redact`] use
//! [`nvisy_core::redaction::ImageReplacement`].
//!
//! [`IndexedHandle::redact`]: crate::core::IndexedHandle::redact

use nvisy_core::modality::{Image, ModalityKind};

use crate::core::Codable;

impl Codable for Image {
    const KIND: ModalityKind = ModalityKind::Image;
}

#[macro_use]
pub(crate) mod macros;
mod image_ops;
pub(crate) mod redact;

#[cfg(feature = "jpeg")]
mod jpeg_handler;
#[cfg(feature = "jpeg")]
mod jpeg_loader;
#[cfg(feature = "png")]
mod png_handler;
#[cfg(feature = "png")]
mod png_loader;
#[cfg(feature = "tiff")]
mod tiff_handler;
#[cfg(feature = "tiff")]
mod tiff_loader;

#[cfg(feature = "jpeg")]
pub use self::jpeg_handler::{JpegHandler, format as jpeg_format};
#[cfg(feature = "jpeg")]
pub use self::jpeg_loader::JpegLoader;
#[cfg(feature = "png")]
pub use self::png_handler::{PngHandler, format as png_format};
#[cfg(feature = "png")]
pub use self::png_loader::PngLoader;
#[cfg(feature = "tiff")]
pub use self::tiff_handler::{TiffHandler, format as tiff_format};
#[cfg(feature = "tiff")]
pub use self::tiff_loader::TiffLoader;
