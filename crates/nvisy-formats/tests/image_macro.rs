//! Smoke test for the `impl_image_handler!` macro exported by
//! `nvisy-codec`.
//!
//! The macro expands to `Handler` + `ImageHandler` + inherent
//! method impls. The PNG handler in `nvisy-formats` consumes it,
//! but having an independent invocation here catches macro path
//! regressions (the macro uses absolute `::nvisy_codec::…` /
//! `::nvisy_core::…` paths, and those need to resolve correctly
//! from any downstream crate).

#![cfg(feature = "png")]

use image::DynamicImage;
use nvisy_codec::handler::Handler;
use nvisy_codec::impl_image_handler;
use nvisy_core::content::ContentSource;
use nvisy_core::media::{DocumentType, ImageFormat};

/// Minimal image-handler struct that satisfies the macro's
/// expectations: `source: ContentSource` and `image: DynamicImage`
/// fields.
struct DummyHandler {
    source: ContentSource,
    image: DynamicImage,
}

impl_image_handler!(
    DummyHandler,
    DocumentType::Image(ImageFormat::Png),
    image::ImageFormat::Png,
    "dummy-handler",
    "dummy.encode"
);

#[test]
fn macro_produces_a_working_handler() {
    let img = DynamicImage::new_rgba8(4, 4);
    let h = DummyHandler::new(img);
    // `document_type` and `source` come from the macro-generated
    // `Handler` impl; this asserts the macro plumbing is intact.
    assert_eq!(h.document_type(), DocumentType::Image(ImageFormat::Png));
    assert_eq!(h.image().width(), 4);
}
