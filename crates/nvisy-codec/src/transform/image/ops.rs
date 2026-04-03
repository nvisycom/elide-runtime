//! [`ImageOps`] trait and implementation for [`DynamicImage`].
//!
//! Each method operates on a single bounding-box region and mutates the
//! image in place (no clone).

use image::DynamicImage;
use image::imageops::FilterType;
use imageproc::filter::gaussian_blur_f32;
use nvisy_ontology::math::{BoundingBoxPixel, Color};

/// Mutating image-transform operations on individual bounding-box regions.
pub trait ImageOps {
    /// Apply a gaussian blur to `region` with the given `sigma`.
    fn apply_gaussian_blur(&mut self, region: &BoundingBoxPixel, sigma: f32);

    /// Fill `region` with a solid `color`.
    fn apply_block_overlay(&mut self, region: &BoundingBoxPixel, color: Color);

    /// Pixelate `region` with the given `block_size`.
    fn apply_pixelate(&mut self, region: &BoundingBoxPixel, block_size: u32);
}

impl ImageOps for DynamicImage {
    fn apply_gaussian_blur(&mut self, region: &BoundingBoxPixel, sigma: f32) {
        let (x, y, w, h) = (region.x, region.y, region.width, region.height);

        let img_w = self.width();
        let img_h = self.height();
        if x >= img_w || y >= img_h {
            return;
        }
        let w = w.min(img_w - x);
        let h = h.min(img_h - y);
        if w == 0 || h == 0 {
            return;
        }

        let sub = self.crop_imm(x, y, w, h);
        let blurred = DynamicImage::ImageRgba8(gaussian_blur_f32(&sub.to_rgba8(), sigma));
        image::imageops::overlay(self, &blurred, x as i64, y as i64);
    }

    fn apply_block_overlay(&mut self, region: &BoundingBoxPixel, color: Color) {
        let (x, y, w, h) = (region.x, region.y, region.width, region.height);

        let img_w = self.width();
        let img_h = self.height();
        if x >= img_w || y >= img_h {
            return;
        }
        let w = w.min(img_w - x);
        let h = h.min(img_h - y);

        let rgba = image::Rgba([color.r, color.g, color.b, 255]);
        let block = image::RgbaImage::from_pixel(w, h, rgba);
        image::imageops::overlay(self, &block, x as i64, y as i64);
    }

    fn apply_pixelate(&mut self, region: &BoundingBoxPixel, block_size: u32) {
        let block_size = block_size.max(1);
        let (x, y, w, h) = (region.x, region.y, region.width, region.height);

        let img_w = self.width();
        let img_h = self.height();
        if x >= img_w || y >= img_h {
            return;
        }
        let w = w.min(img_w - x);
        let h = h.min(img_h - y);
        if w == 0 || h == 0 {
            return;
        }

        let small_w = (w / block_size).max(1);
        let small_h = (h / block_size).max(1);

        let sub = self.crop_imm(x, y, w, h);
        let small = sub.resize_exact(small_w, small_h, FilterType::Nearest);
        let pixelated = small.resize_exact(w, h, FilterType::Nearest);

        image::imageops::overlay(self, &pixelated, x as i64, y as i64);
    }
}
