//! [`ImageRedaction`]: the operator spec an image-modality policy
//! rule carries.
//!
//! Each variant mirrors an elide built-in operator the engine
//! constructs at apply time:
//!
//! - [`ImageRedaction::Erase`] → [`elide::redaction::operators::Erase`]
//! - [`ImageRedaction::Keep`] → [`elide::redaction::operators::Keep`]
//! - [`ImageRedaction::Blur`] → [`elide::redaction::operators::Blur`]
//! - [`ImageRedaction::Pixelate`] →
//!   [`elide::redaction::operators::Pixelate`]
//! - [`ImageRedaction::Blackbox`] →
//!   [`elide::redaction::operators::Blackbox`]

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::schema::ColorSchema;

/// Operator spec a `redact` image rule carries.
#[derive(Debug, Clone, PartialEq)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ImageRedaction {
    /// Clear the matched region.
    Erase,
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
    /// Cover the region with a solid color fill (black by default).
    Blackbox {
        /// Fill color the codec rasterises over the region.
        #[serde(default = "default_blackbox_color")]
        color: ColorSchema,
    },
}

fn default_blur_sigma() -> f32 {
    16.0
}

fn default_pixelate_block() -> u32 {
    16
}

fn default_blackbox_color() -> ColorSchema {
    ColorSchema { r: 0, g: 0, b: 0 }
}
