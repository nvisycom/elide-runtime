//! `ImageStrategy → ImageRedaction` conversion.

use nvisy_codec::handler::{ImageOutput, ImageRedaction};
use nvisy_core::Result;

use crate::policy::ImageStrategy;

/// Convert an [`ImageStrategy`] into a codec [`ImageRedaction`].
/// The mapping is 1:1; no per-strategy infrastructure is required.
pub(crate) fn to_image_redaction(strategy: &ImageStrategy) -> Result<ImageRedaction> {
    let output = match strategy {
        ImageStrategy::Blur { sigma } => ImageOutput::Blur { sigma: *sigma },
        ImageStrategy::Block { color } => ImageOutput::Block { color: *color },
        ImageStrategy::Pixelate { block_size } => ImageOutput::Pixelate {
            block_size: *block_size,
        },
    };
    Ok(ImageRedaction::new(output))
}
