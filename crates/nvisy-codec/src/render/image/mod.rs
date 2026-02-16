//! Image rendering primitives for redaction overlays.
//!
//! Provides gaussian blur, solid-color block overlay, and pixelation
//! functions that operate on [`DynamicImage`] values using bounding-box
//! regions.
//!
//! # Traits
//!
//! [`AsImage`] is the codec extension point: image format handlers
//! implement [`decode`](AsImage::decode) and [`encode`](AsImage::encode)
//! to round-trip through [`DynamicImage`].
//!
//! [`AsRedactableImage`] adds a [`redact`](AsRedactableImage::redact)
//! convenience method that dispatches [`ImageRedactionOutput`] variants
//! to the appropriate rendering primitive. It is automatically
//! implemented for every type that implements [`AsImage`].

mod blur;
mod block;
mod pixelate;

use blur::apply_gaussian_blur;
use block::apply_block_overlay;
use pixelate::apply_pixelate;

use ::image::DynamicImage;
use nvisy_core::error::Error;
use nvisy_core::math::{BoundingBox, BoundingBoxU32};
use crate::render::output::ImageRedactionOutput;

/// A located image redaction: pairs a bounding box with an
/// [`ImageRedactionOutput`] that carries the method-specific parameters.
pub struct ImageRedaction {
    /// Bounding box of the region to redact.
    pub bounding_box: BoundingBox,
    /// The redaction output that determines the rendering method.
    pub output: ImageRedactionOutput,
}

/// Trait for handlers that wrap a raster image.
///
/// Handlers implement [`decode`](Self::decode) and [`encode`](Self::encode)
/// to round-trip through [`DynamicImage`]. See [`AsRedactableImage`] for
/// the higher-level redaction API.
pub trait AsImage: Sized {
    /// Decode the handler's raw bytes into a [`DynamicImage`].
    fn decode(&self) -> Result<DynamicImage, Error>;

    /// Encode a [`DynamicImage`] back into a new handler instance.
    fn encode(image: &DynamicImage) -> Result<Self, Error>;
}

/// Extension trait that adds [`ImageRedactionOutput`]-driven redaction
/// to any [`AsImage`] implementor.
///
/// This trait is automatically implemented for every type that implements
/// [`AsImage`] — handler authors only need to implement [`AsImage`].
pub trait AsRedactableImage: AsImage {
    /// Apply a batch of image redactions, returning a new handler.
    ///
    /// Each [`ImageRedaction`] identifies a bounding box and an
    /// [`ImageRedactionOutput`] that determines the rendering method
    /// (blur, block, pixelate). The image is decoded once, all
    /// redactions are applied in order, and then re-encoded.
    fn redact(&self, redactions: &[ImageRedaction]) -> Result<Self, Error> {
        if redactions.is_empty() {
            return Self::encode(&self.decode()?);
        }

        let mut img = self.decode()?;

        for r in redactions {
            let region = BoundingBoxU32::from(&r.bounding_box);
            let regions = std::slice::from_ref(&region);
            match &r.output {
                ImageRedactionOutput::Blur { sigma } => {
                    img = apply_gaussian_blur(&img, regions, *sigma);
                }
                ImageRedactionOutput::Block { color } => {
                    img = apply_block_overlay(&img, regions, *color);
                }
                ImageRedactionOutput::Pixelate { block_size } => {
                    img = apply_pixelate(&img, regions, *block_size);
                }
                ImageRedactionOutput::Synthesize => {
                    img = apply_block_overlay(&img, regions, [0, 0, 0, 255]);
                }
            }
        }

        Self::encode(&img)
    }
}

/// Blanket implementation: every [`AsImage`] type gets [`AsRedactableImage`] for free.
impl<T: AsImage> AsRedactableImage for T {}
