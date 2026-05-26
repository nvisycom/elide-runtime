//! JPEG handler: holds a decoded image and provides single-location
//! access via [`ImageHandler`].
//!
//! [`ImageHandler::locations`] yields exactly one full-image
//! [`Image`]; [`ImageHandler::read`] returns the current
//! [`DynamicImage`] cropped to the location's
//! bounding box; [`ImageHandler::redact`] applies bounding-box
//! redactions in place.
//!
//! [`DynamicImage`]: image::DynamicImage
//! [`ImageHandler`]: nvisy_codec::handler::ImageHandler
//! [`ImageHandler::locations`]: nvisy_codec::handler::ImageHandler::locations
//! [`ImageHandler::read`]: nvisy_codec::handler::ImageHandler::read
//! [`ImageHandler::redact`]: nvisy_codec::handler::ImageHandler::redact
//! [`Image`]: nvisy_ontology::modality::Image

use nvisy_core::content::ContentSource;

/// Handler for loaded JPEG content.
///
/// Stores the decoded [`DynamicImage`] directly.
/// The raw JPEG bytes can be produced on demand via
/// [`Handler::encode`].
///
/// [`DynamicImage`]: image::DynamicImage
/// [`Handler::encode`]: nvisy_codec::handler::Handler::encode
#[derive(Debug)]
pub struct JpegHandler {
    source: ContentSource,
    image: image::DynamicImage,
}

impl_image_handler!(
    JpegHandler,
    nvisy_core::media::DocumentType::Image(nvisy_core::media::ImageFormat::Jpeg),
    image::ImageFormat::Jpeg,
    "jpeg-handler",
    "jpeg.encode"
);
