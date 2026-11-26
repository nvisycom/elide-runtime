//! [`ImageRedaction`]: the operator spec an image-modality policy
//! rule carries.
//!
//! Each variant mirrors an elide built-in operator the engine
//! constructs at apply time:
//!
//! - [`ImageRedaction::Cover`] → `elide::redaction::operators::Cover`
//! - [`ImageRedaction::Keep`] → `elide::redaction::operators::Keep`
//! - [`ImageRedaction::Blur`] → `elide::redaction::operators::Blur`
//! - [`ImageRedaction::Pixelate`] →
//!   `elide::redaction::operators::Pixelate`
//!
//! There is no `Remove`: an image cannot lose its pixels, so the way
//! to hide a region outright is to cover it.

use elide::primitive::Color;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Operator spec a `redact` image rule carries.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ImageRedaction {
    /// Cover the region with a solid color (black by default).
    Cover {
        /// The color drawn over the region.
        #[serde(default = "default_cover_color")]
        color: Color,
    },
    /// Pass the region through unchanged.
    Keep,
    /// Gaussian-blur the region.
    Blur {
        /// Standard deviation of the Gaussian kernel, in pixels.
        /// Larger is blurrier (and harder to reverse).
        #[serde(default = "default_blur_sigma")]
        sigma: f32,
    },
    /// Mosaic-pixelate the region.
    Pixelate {
        /// Side length of each mosaic block, in pixels. Larger
        /// blocks are coarser (and harder to reverse).
        #[serde(default = "default_pixelate_block")]
        block_size: u32,
    },
}

fn default_blur_sigma() -> f32 {
    16.0
}

fn default_pixelate_block() -> u32 {
    16
}

fn default_cover_color() -> Color {
    Color::BLACK
}
