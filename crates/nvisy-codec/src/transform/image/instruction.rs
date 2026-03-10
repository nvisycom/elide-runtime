//! Image redaction instruction types.

use nvisy_core::math::BoundingBox;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// A located image redaction: pairs a bounding box with
/// an [`ImageOutput`] that carries the method-specific parameters.
pub struct ImageRedaction {
    /// Bounding box of the region to redact.
    pub bounding_box: BoundingBox,
    /// The redaction output that determines the rendering method.
    pub output: ImageOutput,
}

/// Image redaction output — records the method used and its parameters.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "method", rename_all = "snake_case")]
pub enum ImageOutput {
    /// Gaussian blur applied to the region.
    Blur { sigma: f32 },
    /// Opaque block overlay on the region.
    Block { color: [u8; 4] },
    /// Pixelation (mosaic) applied to the region.
    Pixelate { block_size: u32 },
    /// Region replaced with provided image data.
    Replace { data: Vec<u8> },
}
