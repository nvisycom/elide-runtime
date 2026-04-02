//! Image redaction strategies.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::math::Color;

const DEFAULT_BLUR_SIGMA: f32 = 15.0;
const DEFAULT_PIXELATE_BLOCK_SIZE: u32 = 10;

fn default_sigma() -> f32 {
    DEFAULT_BLUR_SIGMA
}
fn default_block_size() -> u32 {
    DEFAULT_PIXELATE_BLOCK_SIZE
}

/// Image redaction strategy with method-specific configuration.
#[derive(Debug, Clone, PartialEq)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(tag = "method", rename_all = "snake_case")]
#[non_exhaustive]
pub enum ImageStrategy {
    /// Apply a gaussian blur.
    Blur {
        /// Blur sigma value.
        #[serde(default = "default_sigma")]
        sigma: f32,
    },
    /// Overlay an opaque block.
    Block {
        /// Color for the block.
        #[serde(default)]
        color: Color,
    },
    /// Apply pixelation (mosaic).
    Pixelate {
        /// Pixel block size.
        #[serde(default = "default_block_size")]
        block_size: u32,
    },
}
