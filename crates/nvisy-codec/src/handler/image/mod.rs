//! Image-modality wire types: [`Codable`] impl, [`ImageData`], and
//! the redaction shapes.
//!
//! The per-modality capability surface lives on the generic
//! [`Handle<Image>`] trait in [`crate::core`]. Concrete per-format
//! implementations (PNG, JPEG, TIFF) live in `nvisy-formats`; the
//! per-region `DynamicImage` redaction helper they share, plus the
//! `impl_image_handler!` macro that builds them, also live there.
//!
//! [`Handle<Image>`]: crate::core::Handle

use nvisy_core::modality::Image;

use crate::core::Codable;

mod image_data;
mod instruction;

pub use self::image_data::ImageData;
pub use self::instruction::{ImageOutput, ImageRedaction};

impl Codable for Image {
    type Data = ImageData;
    type Redaction = ImageRedaction;
}
