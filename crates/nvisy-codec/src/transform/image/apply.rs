//! Helper for applying a batch of [`ImageRedaction`]s to a
//! [`DynamicImage`] in place.

use image::DynamicImage;

use super::instruction::{ImageOutput, ImageRedaction};
use super::ops::ImageOps;

const TARGET: &str = "nvisy_codec::transform::image";

/// Apply a slice of image redactions to `img` in place.
///
/// Each redaction's bounding box is converted to pixel coordinates and
/// the corresponding output method is applied. Replace outputs whose
/// embedded image data fails to decode are skipped with a warning.
pub(crate) fn apply_image_redactions(img: &mut DynamicImage, redactions: &[ImageRedaction]) {
    for redaction in redactions {
        let region = redaction.bounding_box.to_pixel();
        match &redaction.output {
            ImageOutput::Blur { sigma } => {
                img.apply_gaussian_blur(&region, *sigma);
            }
            ImageOutput::Block { color } => {
                img.apply_block_overlay(&region, *color);
            }
            ImageOutput::Pixelate { block_size } => {
                img.apply_pixelate(&region, *block_size);
            }
            ImageOutput::Replace { data } => {
                let replacement = match image::load_from_memory(data) {
                    Ok(r) => r,
                    Err(e) => {
                        tracing::warn!(
                            target: TARGET,
                            region = ?region,
                            error = %e,
                            "failed to decode replacement image data, skipping region"
                        );
                        continue;
                    }
                };
                let resized = replacement.resize_exact(
                    region.width,
                    region.height,
                    image::imageops::FilterType::Lanczos3,
                );
                image::imageops::overlay(img, &resized, region.x as i64, region.y as i64);
            }
        }
    }
}
