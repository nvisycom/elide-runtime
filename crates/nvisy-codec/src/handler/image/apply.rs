//! Helper for applying a single [`ImageRedaction`] to a
//! [`DynamicImage`] in place.

use image::DynamicImage;
use nvisy_ontology::primitive::BoundingBox;

use super::ops::ImageOps;
use crate::handler::{ImageOutput, ImageRedaction};

const TARGET: &str = "nvisy_codec::handler::image";

/// Apply a single redaction to `img` in place at the given bounding
/// box. The bounding box comes from the redaction's containing
/// [`Image`] under the `(location, redaction)` shape — not from
/// the redaction itself.
///
/// Replace outputs whose embedded image data fails to decode are
/// skipped with a warning.
///
/// [`Image`]: nvisy_ontology::modality::Image
pub fn apply_image_redaction(
    img: &mut DynamicImage,
    redaction: &ImageRedaction,
    bounding_box: BoundingBox,
) {
    let region = bounding_box.to_pixel();
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
                    return;
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
