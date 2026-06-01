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
use nvisy_core::media::{DocumentType, ImageFormat};

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
    DocumentType::Image(ImageFormat::Jpeg),
    image::ImageFormat::Jpeg,
    "jpeg-handler",
    "jpeg.encode"
);
