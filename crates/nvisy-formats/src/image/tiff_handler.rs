//! TIFF handler: holds a decoded image and provides single-location
//! access via [`ImageHandler`].
//!
//! [`ImageHandler`]: nvisy_codec::handler::ImageHandler

use nvisy_core::content::ContentSource;

/// Handler for loaded TIFF content.
#[derive(Debug)]
pub struct TiffHandler {
    source: ContentSource,
    image: image::DynamicImage,
}

impl_image_handler!(
    TiffHandler,
    nvisy_core::media::DocumentType::Image(nvisy_core::media::ImageFormat::Tiff),
    image::ImageFormat::Tiff,
    "tiff-handler",
    "tiff.encode"
);
