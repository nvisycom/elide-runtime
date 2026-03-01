//! [`ImageData`]: opaque wrapper around a decoded image.

use derive_more::{From, Into};
use image::DynamicImage;
use nvisy_core::Error;

/// Opaque wrapper around a decoded image.
///
/// Hides the `image::DynamicImage` type so downstream crates
/// do not need a direct `image` dependency.
#[derive(Debug, Clone, From, Into)]
pub struct ImageData(DynamicImage);

impl ImageData {
    /// Encode to PNG bytes.
    pub fn encode_png(&self) -> Result<bytes::Bytes, Error> {
        let mut buf = std::io::Cursor::new(Vec::new());
        self.0
            .write_to(&mut buf, image::ImageFormat::Png)
            .map_err(|e| Error::validation(format!("PNG encode failed: {e}"), "image-data"))?;
        Ok(buf.into_inner().into())
    }

    /// Create a blank RGB image (for tests).
    pub fn new_rgb(width: u32, height: u32) -> Self {
        Self(DynamicImage::new_rgb8(width, height))
    }

    /// Unwrap into the inner `DynamicImage`.
    pub(crate) fn into_inner(self) -> DynamicImage {
        self.0
    }
}
