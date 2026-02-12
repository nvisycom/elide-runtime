//! Gaussian blur for image regions.

use image::DynamicImage;
use imageproc::filter::gaussian_blur_f32;
use nvisy_ontology::ontology::entity::BoundingBox;

/// Apply gaussian blur to the specified regions of an image.
///
/// Each [`BoundingBox`] describes a rectangular region (in pixel coordinates)
/// that will be blurred with the given `sigma` value.
pub fn apply_gaussian_blur(
    image: &DynamicImage,
    regions: &[BoundingBox],
    sigma: f32,
) -> DynamicImage {
    let mut result = image.clone();

    for region in regions {
        let x = region.x.round() as u32;
        let y = region.y.round() as u32;
        let w = region.width.round() as u32;
        let h = region.height.round() as u32;

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

        // Crop the region, blur it, paste it back
        let sub = result.crop_imm(x, y, w, h);
        let blurred = DynamicImage::ImageRgba8(gaussian_blur_f32(&sub.to_rgba8(), sigma));
        image::imageops::overlay(&mut result, &blurred, x as i64, y as i64);
    }

    result
}
