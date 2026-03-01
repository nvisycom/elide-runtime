//! Dots-per-inch resolution type.

use derive_more::{Display, From, Into};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Dots-per-inch resolution for rasterisation and rendering.
///
/// Wraps a [`u16`] to prevent accidental misuse of raw integers as DPI
/// values. Common presets are available as associated constants.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[derive(From, Into, Display, Serialize, Deserialize, JsonSchema)]
#[display("{_0} dpi")]
pub struct Dpi(u16);

impl Dpi {
    /// Screen resolution (72 DPI): matches PDF point units.
    pub const SCREEN: Self = Self(72);

    /// Standard print resolution: 150 DPI.
    pub const PRINT: Self = Self(150);

    /// High-quality OCR resolution: 300 DPI.
    pub const OCR: Self = Self(300);

    /// Create a DPI value from a raw `u16`.
    pub const fn new(value: u16) -> Self {
        Self(value)
    }

    /// Return the raw numeric value.
    pub const fn value(self) -> u16 {
        self.0
    }

    /// Compute the scale factor relative to PDF points (1 pt = 1/72 in).
    pub fn scale_factor(self) -> f32 {
        self.0 as f32 / Self::SCREEN.0 as f32
    }
}

impl Default for Dpi {
    fn default() -> Self {
        Self::OCR
    }
}
