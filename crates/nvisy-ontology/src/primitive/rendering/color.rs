//! RGB color type for image redaction.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// An RGB color value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[derive(Serialize, Deserialize, JsonSchema)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Color {
    pub const BLACK: Self = Self { r: 0, g: 0, b: 0 };
}

impl Default for Color {
    fn default() -> Self {
        Self::BLACK
    }
}
