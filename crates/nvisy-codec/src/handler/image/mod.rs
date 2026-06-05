//! Image-modality wire types: [`Codable`] impl and redaction shapes.
//!
//! The per-modality capability surface lives on the generic
//! [`Handle<Image>`] trait in [`crate::core`]. Per-format
//! implementations (PNG, JPEG, TIFF) and pixel-decode helpers live
//! in `nvisy-formats`.
//!
//! [`Handle<Image>`]: crate::core::Handle

use nvisy_core::modality::{Image, ModalityKind};

use crate::core::Codable;

mod instruction;

pub use self::instruction::{ImageOutput, ImageRedaction};

impl Codable for Image {
    type Redaction = ImageRedaction;

    const KIND: ModalityKind = ModalityKind::Image;
}
