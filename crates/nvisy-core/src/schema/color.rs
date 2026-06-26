//! [`ColorSchema`]: wire shape for [`elide_core::primitive::Color`].

use elide_core::primitive::Color;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Wire-shape proxy for [`elide_core::primitive::Color`] — an 8-bit
/// RGB triple. Used by [`ImageRedaction::Blackbox`] to specify the
/// fill color.
///
/// [`ImageRedaction::Blackbox`]: crate::policy::redaction::ImageRedaction::Blackbox
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "Color")]
pub struct ColorSchema {
    /// Red channel.
    pub r: u8,
    /// Green channel.
    pub g: u8,
    /// Blue channel.
    pub b: u8,
}

impl From<ColorSchema> for Color {
    fn from(s: ColorSchema) -> Self {
        Color {
            r: s.r,
            g: s.g,
            b: s.b,
        }
    }
}

impl From<Color> for ColorSchema {
    fn from(c: Color) -> Self {
        Self {
            r: c.r,
            g: c.g,
            b: c.b,
        }
    }
}
