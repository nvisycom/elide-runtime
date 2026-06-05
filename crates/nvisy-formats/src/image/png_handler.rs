//! PNG handler: holds a decoded image and exposes it as a single
//! full-image chunk via [`Handle<Image>`], with random-access region
//! reads / pixel redactions via [`IndexedHandle<Image>`].
//!
//! [`Handle<Image>`]: nvisy_codec::core::Handle
//! [`IndexedHandle<Image>`]: nvisy_codec::core::IndexedHandle

use nvisy_core::content::ContentSource;

use super::PngLoader;

/// Handler for loaded PNG content. Stores the decoded
/// [`DynamicImage`][di] directly; raw PNG bytes are produced on
/// demand by [`Handler::encode`][he].
///
/// [di]: image::DynamicImage
/// [he]: nvisy_codec::handler::Handler::encode
#[derive(Debug)]
pub struct PngHandler {
    source: ContentSource,
    image: image::DynamicImage,
    yielded: bool,
}

impl_image_handler!(
    handler = PngHandler,
    loader = PngLoader,
    format_id = "nvisy.image.png",
    extensions = ["png"],
    content_types = ["image/png"],
    image_format = image::ImageFormat::Png,
    origin = "png-handler",
    encode_span = "png.encode",
);
