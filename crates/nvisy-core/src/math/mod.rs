//! Spatial and temporal primitive types.
//!
//! Bounding boxes and time spans used across entity locations,
//! rendering, and redaction operations.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// A time interval within an audio or video stream.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TimeSpan {
    /// Start time in seconds from the beginning of the stream.
    pub start_secs: f64,
    /// End time in seconds from the beginning of the stream.
    pub end_secs: f64,
}

/// Axis-aligned bounding box for image-based entity locations.
///
/// Coordinates are `f64` to support both pixel and normalized (0.0–1.0)
/// values from detection models. Use [`BoundingBoxU32`] (or [`Into`])
/// when integer pixel coordinates are needed for rendering.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct BoundingBox {
    /// Horizontal offset of the top-left corner (pixels or normalized).
    pub x: f64,
    /// Vertical offset of the top-left corner (pixels or normalized).
    pub y: f64,
    /// Width of the bounding box.
    pub width: f64,
    /// Height of the bounding box.
    pub height: f64,
}

/// Integer pixel-coordinate bounding box for rendering operations.
///
/// Converted from [`BoundingBox`] by rounding each field to the nearest
/// integer. Use this at the rendering boundary where pixel-exact
/// coordinates are required.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BoundingBoxU32 {
    /// Horizontal offset of the top-left corner in pixels.
    pub x: u32,
    /// Vertical offset of the top-left corner in pixels.
    pub y: u32,
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
}

impl BoundingBox {
    /// Convert to integer pixel coordinates by rounding each field.
    pub fn to_u32(&self) -> BoundingBoxU32 {
        BoundingBoxU32 {
            x: self.x.round() as u32,
            y: self.y.round() as u32,
            width: self.width.round() as u32,
            height: self.height.round() as u32,
        }
    }
}

impl From<&BoundingBox> for BoundingBoxU32 {
    fn from(bb: &BoundingBox) -> Self {
        bb.to_u32()
    }
}

impl From<BoundingBox> for BoundingBoxU32 {
    fn from(bb: BoundingBox) -> Self {
        Self::from(&bb)
    }
}
