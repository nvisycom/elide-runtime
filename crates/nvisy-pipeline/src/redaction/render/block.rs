//! Solid color block overlay for image regions.

use image::{DynamicImage, Rgba, RgbaImage};
use nvisy_ontology::entity::BoundingBox;

/// Apply a solid color block overlay to the specified regions of an image.
///
/// Each [`BoundingBox`] describes a rectangular region (in pixel coordinates)
/// that will be covered with an opaque rectangle of the given `color`.
pub fn apply_block_overlay(
    image: &DynamicImage,
    regions: &[BoundingBox],
    color: Rgba<u8>,
) -> DynamicImage {
    let mut result = image.to_rgba8();
    let img_w = result.width();
    let img_h = result.height();

    for region in regions {
        let x = region.x.round() as u32;
        let y = region.y.round() as u32;
        let w = region.width.round() as u32;
        let h = region.height.round() as u32;

        if x >= img_w || y >= img_h {
            continue;
        }
        let w = w.min(img_w - x);
        let h = h.min(img_h - y);

        let block = RgbaImage::from_pixel(w, h, color);
        image::imageops::overlay(&mut result, &block, x as i64, y as i64);
    }

    DynamicImage::ImageRgba8(result)
}
