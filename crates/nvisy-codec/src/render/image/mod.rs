//! Image rendering primitives for redaction overlays.
//!
//! Provides gaussian blur and solid-color block overlay functions that
//! operate on [`DynamicImage`] values using bounding-box regions.
//!
//! # Trait
//!
//! [`AsImage`] is the main extension point: image format handlers implement
//! [`decode`](AsImage::decode) and [`encode`](AsImage::encode) to round-trip
//! through [`DynamicImage`], and then get [`blur`](AsImage::blur) and
//! [`block`](AsImage::block) convenience methods for free via default
//! implementations.
//!
//! # Sub-modules
//!
//! | Module | Description |
//! |--------|-------------|
//! | [`blur`] | Gaussian blur rendering |
//! | [`block`] | Solid-color block overlay rendering |

mod blur;
mod block;

pub use blur::apply_gaussian_blur;
pub use block::apply_block_overlay;

use ::image::DynamicImage;
use nvisy_core::error::Error;
use nvisy_ontology::entity::BoundingBox;

/// Trait for handlers that wrap a raster image.
///
/// Provides [`decode`](Self::decode) / [`encode`](Self::encode) for
/// round-tripping through [`DynamicImage`], plus convenience methods for
/// applying blur and block-overlay redactions in a single call.
pub trait AsImage: Sized {
    /// Decode the handler's raw bytes into a [`DynamicImage`].
    fn decode(&self) -> Result<DynamicImage, Error>;

    /// Encode a [`DynamicImage`] back into a new handler instance.
    fn encode(image: &DynamicImage) -> Result<Self, Error>;

    /// Apply gaussian blur to the given bounding-box regions.
    fn blur(&self, regions: &[BoundingBox], sigma: f32) -> Result<Self, Error> {
        let img = apply_gaussian_blur(&self.decode()?, regions, sigma);
        Self::encode(&img)
    }

    /// Apply a solid-color block overlay to the given bounding-box regions.
    fn block(&self, regions: &[BoundingBox], color: [u8; 4]) -> Result<Self, Error> {
        let img = apply_block_overlay(&self.decode()?, regions, color);
        Self::encode(&img)
    }
}
