//! Pixelation (mosaic) rendering for bounding-box regions.
//!
//! The algorithm works per-region:
//! 1. Crop the rectangular area from the source image.
//! 2. Downscale the crop by the block size factor.
//! 3. Upscale back to the original size using nearest-neighbour sampling.
//! 4. Paste the pixelated sub-image back over the original at the same position.
//!
//! Regions are clamped to image bounds so that out-of-range coordinates are
//! silently ignored rather than causing a panic.

use ::image::imageops::FilterType;
use ::image::DynamicImage;
use nvisy_core::math::BoundingBoxU32;

/// Apply pixelation (mosaic effect) to the specified regions of an image.
///
/// Each [`BoundingBoxU32`] describes a rectangular region (in pixel
/// coordinates) that will be pixelated. The `block_size` controls mosaic
/// granularity — higher values produce larger, blockier pixels.
pub fn apply_pixelate(
    image: &DynamicImage,
    regions: &[BoundingBoxU32],
    block_size: u32,
) -> DynamicImage {
    let mut result = image.clone();
    let block_size = block_size.max(1);

    for region in regions {
        let (x, y, w, h) = (region.x, region.y, region.width, region.height);

        // Clamp to image bounds
        let img_w = result.width();
        let img_h = result.height();
        if x >= img_w || y >= img_h {
            continue;
        }
        let w = w.min(img_w - x);
        let h = h.min(img_h - y);
        if w == 0 || h == 0 {
            continue;
        }

        let small_w = (w / block_size).max(1);
        let small_h = (h / block_size).max(1);

        let sub = result.crop_imm(x, y, w, h);
        let small = sub.resize_exact(small_w, small_h, FilterType::Nearest);
        let pixelated = small.resize_exact(w, h, FilterType::Nearest);

        ::image::imageops::overlay(&mut result, &pixelated, x as i64, y as i64);
    }

    result
}
