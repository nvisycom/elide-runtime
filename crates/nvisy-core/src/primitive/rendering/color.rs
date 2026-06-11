//! RGB color type for image redaction.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// An RGB color value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[derive(Serialize, Deserialize, JsonSchema)]
pub struct Color {
    /// Red channel (`0..=255`).
    pub r: u8,
    /// Green channel (`0..=255`).
    pub g: u8,
    /// Blue channel (`0..=255`).
    pub b: u8,
}

impl Color {
    /// Opaque black (`#000000`). The redaction default — image
    /// handlers fall back to this when an operator omits a fill.
    pub const BLACK: Self = Self { r: 0, g: 0, b: 0 };
}

impl Default for Color {
    fn default() -> Self {
        Self::BLACK
    }
}
