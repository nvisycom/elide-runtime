//! JPEG handler: holds a decoded image and provides single-span access
//! via [`ImageHandler`](crate::handler::ImageHandler).
//!
//! # Span model
//!
//! [`ImageHandler::image_spans`](crate::handler::ImageHandler::image_spans)
//! yields exactly one [`Span`] whose data is the current
//! [`DynamicImage`](image::DynamicImage).
//! [`ImageHandler::edit_images`](crate::handler::ImageHandler::edit_images)
//! replaces the image in-place.

use nvisy_core::path::ContentSource;

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
    nvisy_core::fs::DocumentType::Image(nvisy_core::fs::ImageFormat::Jpeg),
    image::ImageFormat::Jpeg,
    "jpeg-handler",
    "jpeg.encode"
);
