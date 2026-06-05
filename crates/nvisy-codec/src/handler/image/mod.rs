//! Image-modality wire types: [`Codable`] impl.
//!
//! The per-modality capability surface lives on the generic
//! [`Handle<Image>`] trait in [`crate::core`]. Per-format
//! implementations (PNG, JPEG, TIFF) and pixel-decode helpers live
//! in `nvisy-formats`.
//!
//! Replacements written during [`IndexedHandle::redact`] use
//! [`nvisy_core::redaction::ImageReplacement`].
//!
//! [`Handle<Image>`]: crate::core::Handle
//! [`IndexedHandle::redact`]: crate::core::IndexedHandle::redact

use nvisy_core::modality::{Image, ModalityKind};

use crate::core::Codable;

impl Codable for Image {
    const KIND: ModalityKind = ModalityKind::Image;
}
