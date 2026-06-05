//! Text-modality wire types: [`Codable`] impl + redaction shapes.
//!
//! The per-modality capability surface lives on the generic
//! [`Handle<Text>`] trait in [`crate::core`]. Concrete per-format
//! implementations (TXT, JSON, Markdown, HTML) live in
//! `nvisy-formats`; the byte-level `&mut String` redaction helper
//! they share lives there too.
//!
//! [`Handle<Text>`]: crate::core::Handle

use nvisy_core::modality::{ModalityKind, Text};

use crate::core::Codable;

mod instruction;

pub use self::instruction::{TextOutput, TextRedaction};

impl Codable for Text {
    type Redaction = TextRedaction;

    const KIND: ModalityKind = ModalityKind::Text;
}
