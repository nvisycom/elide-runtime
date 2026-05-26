//! Text-modality codec types: [`Codable`] impl, redaction shapes,
//! and the `apply_text_redaction` helper.
//!
//! The per-modality capability surface lives on the generic
//! [`Handle<Text>`] trait in [`super::handle`]. Concrete per-format
//! implementations (TXT, JSON, Markdown, HTML) live in
//! `nvisy-formats`.
//!
//! [`Handle<Text>`]: super::Handle

use nvisy_ontology::modality::Text;

use super::Codable;

mod apply;
mod instruction;
mod text_data;

pub use self::apply::apply_text_redaction;
pub use self::instruction::{TextOutput, TextRedaction};
pub use self::text_data::TextData;

impl Codable for Text {
    type Data = TextData;
    type Redaction = TextRedaction;
}
