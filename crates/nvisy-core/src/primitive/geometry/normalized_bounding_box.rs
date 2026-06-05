//! Normalised `[0, 1]` axis-aligned bounding box.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::{BoundingBox, Dimensions};

/// Axis-aligned bounding box in normalised `[0, 1]` coordinates.
///
/// Used at API boundaries where pixel dimensions are unknown to
/// the producer — most commonly the output of vision-language
/// models that don't see the original image's pixel size.
/// `(0, 0)` is the top-left corner of the image, `(1, 1)` is the
/// bottom-right.
///
/// Field values are **not** clamped to `[0, 1]`; the type carries
/// the *intent* of normalised coordinates, not a hard invariant.
/// Producers should keep values in range; conversion to pixel
/// space ([`NormalizedBoundingBox::to_pixel`]) is mechanical
/// multiplication regardless.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
#[derive(Serialize, Deserialize, JsonSchema)]
pub struct NormalizedBoundingBox {
    /// Top-left x in `[0, 1]` (fraction of image width).
    pub x: f64,
    /// Top-left y in `[0, 1]` (fraction of image height).
    pub y: f64,
    /// Width in `[0, 1]` (fraction of image width).
    pub width: f64,
    /// Height in `[0, 1]` (fraction of image height).
    pub height: f64,
}

impl NormalizedBoundingBox {
    /// Create a normalised bounding box from explicit fields.
    pub fn new(x: f64, y: f64, width: f64, height: f64) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// Convert to pixel-space [`BoundingBox`] by multiplying each
    /// dimension by the corresponding axis of `dims`.
    ///
    /// Out-of-range input ([0, 1] violations) propagates as
    /// out-of-bounds pixel values — caller's responsibility to
    /// keep producers honest.
    pub fn to_pixel(&self, dims: Dimensions) -> BoundingBox {
        BoundingBox::new(
            self.x * f64::from(dims.width),
            self.y * f64::from(dims.height),
            self.width * f64::from(dims.width),
            self.height * f64::from(dims.height),
        )
    }
}
