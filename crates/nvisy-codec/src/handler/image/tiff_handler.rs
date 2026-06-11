//! TIFF handler: see [`super::png_handler`] for the shared shape.

use super::TiffLoader;
use crate::content::ContentSource;

/// Handler for loaded TIFF content. Holds the decoded
/// [`image::DynamicImage`] and a streaming cursor; the macro-generated
/// `Handler<Image>` impl encodes back to TIFF on demand.
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
