//! JPEG handler — holds a decoded image and provides single-span access
//! via [`Handler`].
//!
//! # Span model
//!
//! [`Handler::view_spans`] yields exactly one [`Span`] whose data is the
//! current [`DynamicImage`](image::DynamicImage).
//! [`Handler::edit_spans`] replaces the image in-place.

use super::impl_image_handler;

/// Handler for loaded JPEG content.
///
/// Stores the decoded [`DynamicImage`](image::DynamicImage) directly.
/// The raw JPEG bytes can be produced on demand via
/// [`Handler::encode`](crate::handler::Handler::encode).
#[derive(Debug, Clone)]
pub struct JpegHandler {
    image: image::DynamicImage,
}

impl_image_handler!(
    JpegHandler,
    nvisy_core::fs::DocumentType::Jpeg,
    image::ImageFormat::Jpeg,
    "jpeg-handler"
);
