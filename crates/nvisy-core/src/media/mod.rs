//! Media format classification types.
//!
//! - [`DocumentType`]: top-level document classification.
//! - `document_format::*`: per-category leaf format enums
//!   ([`ImageFormat`], [`AudioFormat`], …) nested inside
//!   [`DocumentType`] variants.

mod document_format;
mod document_type;

pub use self::document_format::{
    AudioFormat, ImageFormat, SpreadsheetFormat, TextFormat, WordFormat,
};
pub use self::document_type::DocumentType;
