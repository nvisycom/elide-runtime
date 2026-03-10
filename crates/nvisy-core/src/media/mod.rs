//! Media format classification types.
//!
//! - [`ContentKind`]: High-level content category (text, image, document, …)
//! - [`DocumentType`]: Specific document format with sub-format enums

mod content_kind;
mod document_type;

pub use content_kind::ContentKind;
pub use document_type::{
    AudioFormat, DocumentType, ImageFormat, PresentationFormat, SpreadsheetFormat, TextFormat,
    WordFormat,
};
