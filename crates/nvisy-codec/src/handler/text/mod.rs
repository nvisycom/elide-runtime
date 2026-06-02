//! Text-modality wire types: [`Codable`] impl + redaction shapes.
//!
//! The per-modality capability surface lives on the generic
//! [`Handle<Text>`] trait in [`crate::core`]. Concrete per-format
//! implementations (TXT, JSON, Markdown, HTML) live in
//! `nvisy-formats`; the byte-level `&mut String` redaction helper
//! they share lives there too.
//!
//! [`Handle<Text>`]: crate::core::Handle

use nvisy_core::modality::Text;

use crate::core::Codable;

mod instruction;
mod text_data;

pub use self::instruction::{TextOutput, TextRedaction};
pub use self::text_data::TextData;

impl Codable for Text {
    type Data = TextData;
    type Redaction = TextRedaction;
}
