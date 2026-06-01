//! Image redaction strategies.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::modality::{LeakProfile, RedactionStrategy};
use crate::primitive::Color;

const DEFAULT_BLUR_SIGMA: f32 = 15.0;
const DEFAULT_PIXELATE_BLOCK_SIZE: u32 = 10;

fn default_sigma() -> f32 {
    DEFAULT_BLUR_SIGMA
}
fn default_block_size() -> u32 {
    DEFAULT_PIXELATE_BLOCK_SIZE
}

/// Image redaction strategy with method-specific configuration.
///
/// The [`Default`] impl returns an opaque [`Block`] in [`Color::default`]
/// (black) — the most-destructive option short of removing the region.
///
/// [`Block`]: ImageStrategy::Block
/// [`Color::default`]: crate::primitive::Color::default
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

impl Default for ImageStrategy {
    fn default() -> Self {
        Self::Block {
            color: Color::default(),
        }
    }
}

/// Parameter-less tag for each [`ImageStrategy`] variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ImageMethodTag {
    /// Tag for [`ImageStrategy::Blur`].
    Blur,
    /// Tag for [`ImageStrategy::Block`].
    Block,
    /// Tag for [`ImageStrategy::Pixelate`].
    Pixelate,
}

impl RedactionStrategy for ImageStrategy {
    /// All image strategies are [`Partial`]: the original pixels are
    /// gone, but the bounding box stays observable. No image strategy
    /// is currently [`Irrecoverable`]; reaching that would require
    /// cropping the region out entirely, which the codec doesn't
    /// model today.
    ///
    /// [`Partial`]: LeakProfile::Partial
    /// [`Irrecoverable`]: LeakProfile::Irrecoverable
    fn leak_profile(&self) -> LeakProfile {
        match self {
            Self::Blur { .. } | Self::Block { .. } | Self::Pixelate { .. } => LeakProfile::Partial,
        }
    }
}
