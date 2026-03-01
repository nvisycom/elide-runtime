//! Image redaction output type.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Image redaction output — records the method used and its parameters.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "method", rename_all = "snake_case")]
pub enum ImageRedactionOutput {
    /// Gaussian blur applied to the region.
    Blur { sigma: f32 },
    /// Opaque block overlay on the region.
    Block { color: [u8; 4] },
    /// Pixelation (mosaic) applied to the region.
    Pixelate { block_size: u32 },
    /// Region replaced with provided image data.
    Replace { data: Vec<u8> },
}
