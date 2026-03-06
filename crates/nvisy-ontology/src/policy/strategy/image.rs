//! Image redaction strategies.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

const DEFAULT_BLUR_SIGMA: f32 = 15.0;
const DEFAULT_BLOCK_COLOR: [u8; 4] = [0, 0, 0, 255];
const DEFAULT_PIXELATE_BLOCK_SIZE: u32 = 10;

fn default_sigma() -> f32 {
    DEFAULT_BLUR_SIGMA
}
fn default_block_color() -> [u8; 4] {
    DEFAULT_BLOCK_COLOR
}
fn default_block_size() -> u32 {
    DEFAULT_PIXELATE_BLOCK_SIZE
}

/// Image redaction strategy with method-specific configuration.
#[derive(Debug, Clone, PartialEq)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(tag = "method", rename_all = "snake_case")]
pub enum ImageStrategy {
    /// Apply a gaussian blur.
    Blur {
        /// Blur sigma value.
        #[serde(default = "default_sigma")]
        sigma: f32,
    },
    /// Overlay an opaque block.
    Block {
        /// RGBA color for the block.
        #[serde(default = "default_block_color")]
        color: [u8; 4],
    },
    /// Apply pixelation (mosaic).
    Pixelate {
        /// Pixel block size.
        #[serde(default = "default_block_size")]
        block_size: u32,
    },
}
