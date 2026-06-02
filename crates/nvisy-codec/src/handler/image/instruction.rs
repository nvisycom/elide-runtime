//! Image redaction instruction types.

use nvisy_core::primitive::Color;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// An image redaction: the *how*. The *where* (bounding box, page
/// number, image id) lives on the containing [`Image`] via
/// [`Redactions`]'s `(S, R)` pairs.
///
/// [`Image`]: nvisy_core::modality::Image
/// [`Redactions`]: crate::core::Redactions
#[derive(Debug, Clone, PartialEq)]
pub struct ImageRedaction {
    /// The redaction output that determines the rendering method.
    pub(crate) output: ImageOutput,
}

impl ImageRedaction {
    /// Create a new image redaction with the given output.
    pub fn new(output: ImageOutput) -> Self {
        Self { output }
    }

    /// The redaction output that determines the rendering method.
    pub fn output(&self) -> &ImageOutput {
        &self.output
    }
}

/// Image redaction output: records the method used and its parameters.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "method", rename_all = "snake_case")]
pub enum ImageOutput {
    /// Gaussian blur applied to the region.
    Blur { sigma: f32 },
    /// Opaque block overlay on the region.
    Block { color: Color },
    /// Pixelation (mosaic) applied to the region.
    Pixelate { block_size: u32 },
    /// Region replaced with provided image data.
    Replace { data: Vec<u8> },
}
