//! JPEG handler: holds a decoded image and provides single-location
//! access via [`ImageHandler`].
//!
//! [`ImageHandler::locations`] yields exactly one full-image
//! [`ImageLocation`]; [`ImageHandler::read`] returns the current
//! [`DynamicImage`](image::DynamicImage) cropped to the location's
//! bounding box; [`ImageHandler::redact`] applies bounding-box
//! redactions in place.
//!
//! [`ImageHandler`]: crate::handler::ImageHandler
//! [`ImageHandler::locations`]: crate::handler::ImageHandler::locations
//! [`ImageHandler::read`]: crate::handler::ImageHandler::read
//! [`ImageHandler::redact`]: crate::handler::ImageHandler::redact
//! [`ImageLocation`]: nvisy_ontology::entity::ImageLocation

use nvisy_core::content::ContentSource;

use super::impl_image_handler;

/// Handler for loaded JPEG content.
///
/// Stores the decoded [`DynamicImage`](image::DynamicImage) directly.
/// The raw JPEG bytes can be produced on demand via
/// [`Handler::encode`](crate::handler::Handler::encode).
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
