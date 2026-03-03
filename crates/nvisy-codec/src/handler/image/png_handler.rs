//! PNG handler: holds a decoded image and provides single-span access
//! via [`Handler`].
//!
//! # Span model
//!
//! [`Handler::view_spans`] yields exactly one [`Span`] whose data is the
//! current [`DynamicImage`](image::DynamicImage).
//! [`Handler::edit_spans`] replaces the image in-place.

use super::impl_image_handler;

/// Handler for loaded PNG content.
///
/// Stores the decoded [`DynamicImage`](image::DynamicImage) directly.
/// The raw PNG bytes can be produced on demand via
/// [`Handler::encode`](crate::handler::Handler::encode).
#[derive(Debug)]
pub struct PngHandler {
    source: nvisy_core::path::ContentSource,
    image: image::DynamicImage,
}

impl_image_handler!(
    PngHandler,
    nvisy_core::fs::DocumentType::Png,
    image::ImageFormat::Png,
    "png-handler",
    "png.encode"
);
