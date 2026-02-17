//! Solid-color block overlay rendering for bounding-box regions.
//!
//! For each region the algorithm creates an opaque [`RgbaImage`] rectangle
//! filled with the requested colour and composites it onto the target image
//! using alpha-over blending. Regions are clamped to image bounds.

use ::image::{DynamicImage, Rgba, RgbaImage};
use nvisy_core::math::BoundingBoxU32;

/// Apply a solid color block overlay to the specified regions of an image.
///
/// Each [`BoundingBoxU32`] describes a rectangular region (in pixel
/// coordinates) that will be covered with an opaque rectangle of the
/// given `color` (RGBA).
pub fn apply_block_overlay(
    image: &DynamicImage,
    regions: &[BoundingBoxU32],
    color: [u8; 4],
) -> DynamicImage {
    let color = Rgba(color);
    let mut result = image.to_rgba8();
    let img_w = result.width();
    let img_h = result.height();

    for region in regions {
        let (x, y, w, h) = (region.x, region.y, region.width, region.height);

        if x >= img_w || y >= img_h {
            continue;
        }
        let w = w.min(img_w - x);
        let h = h.min(img_h - y);

        let block = RgbaImage::from_pixel(w, h, color);
        ::image::imageops::overlay(&mut result, &block, x as i64, y as i64);
    }

    DynamicImage::ImageRgba8(result)
}
