//! Gaussian blur rendering for bounding-box regions.
//!
//! The algorithm works per-region:
//! 1. Crop the rectangular area from the source image.
//! 2. Apply a gaussian blur with the given `sigma` to the cropped sub-image.
//! 3. Paste the blurred sub-image back over the original at the same position.
//!
//! Regions are clamped to image bounds so that out-of-range coordinates are
//! silently ignored rather than causing a panic.

use ::image::DynamicImage;
use imageproc::filter::gaussian_blur_f32;
use nvisy_core::math::BoundingBoxU32;

/// Apply gaussian blur to the specified regions of an image.
///
/// Each [`BoundingBoxU32`] describes a rectangular region (in pixel
/// coordinates) that will be blurred with the given `sigma` value.
pub fn apply_gaussian_blur(
    image: &DynamicImage,
    regions: &[BoundingBoxU32],
    sigma: f32,
) -> DynamicImage {
    let mut result = image.clone();

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

        let sub = result.crop_imm(x, y, w, h);
        let blurred = DynamicImage::ImageRgba8(gaussian_blur_f32(&sub.to_rgba8(), sigma));
        ::image::imageops::overlay(&mut result, &blurred, x as i64, y as i64);
    }

    result
}
