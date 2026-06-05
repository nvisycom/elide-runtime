//! TIFF handler: see [`super::png_handler`] for the shared shape.

use nvisy_core::content::ContentSource;

use super::TiffLoader;

#[derive(Debug)]
pub struct TiffHandler {
    source: ContentSource,
    image: image::DynamicImage,
    yielded: bool,
}

impl_image_handler!(
    handler = TiffHandler,
    loader = TiffLoader,
    format_id = "nvisy.image.tiff",
    extensions = ["tif", "tiff"],
    content_types = ["image/tiff"],
    image_format = image::ImageFormat::Tiff,
    origin = "tiff-handler",
    encode_span = "tiff.encode",
);
