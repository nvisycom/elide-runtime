//! Image-format implementations: PNG, JPEG, TIFF.

#[macro_use]
mod macros;
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
pub use self::jpeg_handler::JpegHandler;
#[cfg(feature = "jpeg")]
pub use self::jpeg_loader::{JpegLoader, JpegParams};
#[cfg(feature = "png")]
pub use self::png_handler::PngHandler;
#[cfg(feature = "png")]
pub use self::png_loader::{PngLoader, PngParams};
#[cfg(feature = "tiff")]
pub use self::tiff_handler::TiffHandler;
#[cfg(feature = "tiff")]
pub use self::tiff_loader::{TiffLoader, TiffParams};
