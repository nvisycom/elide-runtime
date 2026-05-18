//! PNG handler: holds a decoded image and provides single-span access
//! via [`ImageHandler`](crate::handler::ImageHandler).
//!
//! # Span model
//!
//! [`ImageHandler::image_spans`](crate::handler::ImageHandler::image_spans)
//! yields exactly one [`Span`] whose data is the current
//! [`DynamicImage`](image::DynamicImage).
//! [`ImageHandler::edit_images`](crate::handler::ImageHandler::edit_images)
//! replaces the image in-place.
//!
//! [`Span`]: crate::document::Span

use nvisy_core::content::ContentSource;

use super::impl_image_handler;

/// Handler for loaded PNG content.
///
/// Stores the decoded [`DynamicImage`](image::DynamicImage) directly.
/// The raw PNG bytes can be produced on demand via
/// [`Handler::encode`](crate::handler::Handler::encode).
#[derive(Debug)]
pub struct PngHandler {
    source: ContentSource,
    image: image::DynamicImage,
}

impl_image_handler!(
    PngHandler,
    nvisy_core::media::DocumentType::Image(nvisy_core::media::ImageFormat::Png),
    image::ImageFormat::Png,
    "png-handler",
    "png.encode"
);
