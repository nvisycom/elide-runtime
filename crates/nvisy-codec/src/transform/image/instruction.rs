//! Image redaction instruction types.

use nvisy_ontology::primitive::{BoundingBox, Color};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::transform::Mergeable;

/// An image redaction targeting a bounding box within its containing span.
///
/// Span identity is supplied externally via [`Redactions`] — this
/// struct only carries the bounding box and the rendering method.
///
/// [`Redactions`]: crate::transform::Redactions
#[derive(Debug, Clone, PartialEq)]
pub struct ImageRedaction {
    /// Bounding box of the region to redact within the span.
    pub(crate) bounding_box: BoundingBox,
    /// The redaction output that determines the rendering method.
    pub(crate) output: ImageOutput,
}

impl ImageRedaction {
    /// Create a new image redaction.
    pub fn new(bounding_box: BoundingBox, output: ImageOutput) -> Self {
        Self {
            bounding_box,
            output,
        }
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
    fn overlaps(&self, other: &Self) -> bool {
        self.bounding_box.overlaps(&other.bounding_box)
    }

    /// Merge two overlapping image redactions.
    ///
    /// Returns `Some` only when both share the same [`ImageOutput`]
    /// (method *and* parameters) — the merged redaction unions the
    /// bounding boxes. Returns `None` when the methods differ (e.g.
    /// `Blur { sigma: 5.0 }` vs `Pixelate { block_size: 10 }`).
    fn try_merge(self, other: Self) -> Option<Self> {
        if self.output != other.output {
            return None;
        }
        Some(Self {
            bounding_box: self.bounding_box.union(&other.bounding_box),
            output: self.output,
        })
    }
}
