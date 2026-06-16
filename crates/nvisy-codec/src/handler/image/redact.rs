//! Per-region image redaction helper shared by every image handler
//! (PNG, JPEG, TIFF — all reduce to single-image `DynamicImage`s).
//!
//! Applies one [`ImageReplacement`] in place against a bounding box.
//! The per-output pixel transforms (blur, block, pixelate) live on
//! the [`ImageOps`] trait in [`super::image_ops`].

use image::DynamicImage;
use image::imageops::FilterType;
use nvisy_core::primitive::BoundingBox;
use nvisy_core::redaction::ImageReplacement;

use super::image_ops::ImageOps;

const TARGET: &str = "nvisy_codec::handler::image::redact";

/// Apply a single replacement to `img` in place at the given bounding
/// box.
///
/// `Replace` variants whose embedded image data fails to decode are
/// skipped with a warning.
pub(crate) fn apply(
    img: &mut DynamicImage,
    replacement: &ImageReplacement,
    bounding_box: BoundingBox,
) {
    let region = bounding_box.to_pixel();
    match replacement {
        ImageReplacement::Blur { sigma } => {
            img.apply_gaussian_blur(&region, *sigma);
        }
        ImageReplacement::Block { color } => {
            img.apply_block_overlay(&region, *color);
        }
        ImageReplacement::Pixelate { block_size } => {
            img.apply_pixelate(&region, *block_size);
        }
        ImageReplacement::Replace { data } => {
            let replacement_img = match image::load_from_memory(data) {
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
            let resized =
                replacement_img.resize_exact(region.width, region.height, FilterType::Lanczos3);
            image::imageops::overlay(img, &resized, region.x as i64, region.y as i64);
        }
    }
}
