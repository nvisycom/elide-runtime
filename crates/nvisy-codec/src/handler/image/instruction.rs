//! Image redaction instruction types.

use nvisy_ontology::primitive::Color;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::handler::Mergeable;

/// An image redaction: the *how*. The *where* (bounding box, page
/// number, image id) lives on the containing [`ImageLocation`] via
/// [`Redactions`]'s `(S, R)` pairs.
///
/// [`ImageLocation`]: nvisy_ontology::entity::ImageLocation
/// [`Redactions`]: crate::handler::Redactions
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

impl Mergeable for ImageRedaction {
    /// Combine two redactions that target overlapping locations.
    /// Returns `Some` only when the outputs match (method *and*
    /// parameters); a Blur and a Pixelate cannot be reconciled.
    fn try_merge(self, other: Self) -> Option<Self> {
        (self.output == other.output).then_some(self)
    }
}
