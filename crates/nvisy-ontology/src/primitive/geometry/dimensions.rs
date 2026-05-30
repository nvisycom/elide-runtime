//! Image / canvas dimensions in integer pixels.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Pixel dimensions of an image or any 2D canvas.
///
/// Used to convert between normalized `[0, 1]` coordinates (what
/// vision models typically emit) and absolute pixel coordinates
/// (what renderers and the rest of our pipeline consume). See
/// [`NormalizedBoundingBox::to_pixel`] for the conversion.
///
/// [`NormalizedBoundingBox::to_pixel`]: super::NormalizedBoundingBox::to_pixel
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[derive(Serialize, Deserialize, JsonSchema)]
pub struct Dimensions {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
}

impl Dimensions {
    /// Create a `Dimensions` from explicit width and height.
    pub fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }
}

impl From<(u32, u32)> for Dimensions {
    fn from((width, height): (u32, u32)) -> Self {
        Self { width, height }
    }
}
