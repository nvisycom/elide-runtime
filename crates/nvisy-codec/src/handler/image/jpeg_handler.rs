//! JPEG handler: see [`super::png_handler`] for the shared shape.

use super::JpegLoader;
use crate::content::ContentSource;

#[derive(Debug)]
pub struct JpegHandler {
    source: ContentSource,
    image: image::DynamicImage,
    yielded: bool,
}

impl_image_handler!(
    handler = JpegHandler,
    loader = JpegLoader,
    format_id = "nvisy.image.jpeg",
    extensions = ["jpg", "jpeg"],
    content_types = ["image/jpeg"],
    image_format = image::ImageFormat::Jpeg,
    origin = "jpeg-handler",
    encode_span = "jpeg.encode",
);
