//! Image-modality codec types: [`Codable`] impl, redaction shapes,
//! `ImageData`, ops helpers, and the `apply_image_redaction` +
//! `impl_image_handler!` shared utilities.
//!
//! The per-modality capability surface lives on the generic
//! [`Handle<Image>`] trait in [`super::handle`]. Concrete per-format
//! implementations (PNG, JPEG, TIFF) live in `nvisy-formats`.
//!
//! [`Handle<Image>`]: super::Handle

use nvisy_ontology::modality::Image;

use super::Codable;

mod apply;
mod image_data;
mod image_handler_macro;
mod instruction;
mod ops;

pub use self::apply::apply_image_redaction;
pub use self::image_data::ImageData;
pub use self::instruction::{ImageOutput, ImageRedaction};
// `impl_image_handler!` is `#[macro_export]`-ed in
// `image_handler_macro.rs`, so it lives at
// `::nvisy_codec::impl_image_handler`.

impl Codable for Image {
    type Data = ImageData;
    type Redaction = ImageRedaction;
}
