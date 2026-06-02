//! [`ImageData`]: opaque wrapper around a decoded image.

use std::io::Cursor;

use derive_more::{From, Into};
use image::DynamicImage;
use nvisy_core::Error;
use nvisy_core::content::ContentData;
use nvisy_core::primitive::Dimensions;

/// Opaque wrapper around a decoded image.
///
/// Hides the `image::DynamicImage` type so downstream crates
/// do not need a direct `image` dependency.
#[derive(Debug, Clone, From, Into)]
pub struct ImageData(DynamicImage);

impl ImageData {
    /// Decode raw bytes into an [`ImageData`].
    ///
    /// Records `width` and `height` on the current tracing span if set.
    pub fn decode(content: &ContentData, origin: &str) -> Result<Self, Error> {
        let raw = content.to_bytes();
        let img = image::load_from_memory(&raw).map_err(|e| {
            Error::validation(format!("image decode failed: {e}"), origin.to_owned())
        })?;
        tracing::Span::current().record("width", img.width());
        tracing::Span::current().record("height", img.height());
        Ok(Self(img))
    }

    /// Pixel dimensions of the decoded image.
    pub fn dimensions(&self) -> Dimensions {
        Dimensions::new(self.0.width(), self.0.height())
    }

    /// Encode to PNG bytes.
    pub fn encode_png(&self) -> Result<bytes::Bytes, Error> {
        let mut buf = Cursor::new(Vec::new());
        self.0
            .write_to(&mut buf, image::ImageFormat::Png)
            .map_err(|e| Error::validation(format!("PNG encode failed: {e}"), "image-data"))?;
        Ok(buf.into_inner().into())
    }

    /// Unwrap into the inner `DynamicImage`.
    pub fn into_inner(self) -> DynamicImage {
        self.0
    }
}
