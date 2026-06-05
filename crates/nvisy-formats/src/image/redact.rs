//! Per-region image redaction helper shared by every image handler
//! (PNG, JPEG, TIFF — all reduce to single-image `DynamicImage`s).
//!
//! Applies one [`ImageRedaction`] in place against a bounding box.
//! The per-output pixel transforms (blur, block, pixelate) live on
//! the [`ImageOps`] trait in [`super::image_ops`].

use image::DynamicImage;
use nvisy_codec::handler::{ImageOutput, ImageRedaction};
use nvisy_core::primitive::BoundingBox;

use super::image_ops::ImageOps;

const TARGET: &str = "nvisy_formats::image";

/// Apply a single redaction to `img` in place at the given bounding
/// box.
///
/// `Replace` outputs whose embedded image data fails to decode are
/// skipped with a warning.
pub(crate) fn apply(img: &mut DynamicImage, redaction: &ImageRedaction, bounding_box: BoundingBox) {
    let region = bounding_box.to_pixel();
    match redaction.output() {
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
