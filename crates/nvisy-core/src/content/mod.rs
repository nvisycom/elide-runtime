//! Content data containers, metadata, source identity, and media
//! format classification.
//!
//! - [`ContentData`]: Raw content bytes with source identity
//! - [`ContentMetadata`]: MIME type, filename, and descriptive attributes
//! - [`Content`]: [`ContentData`] paired with optional [`ContentMetadata`]
//! - [`ContentSource`]: UUIDv7-based content identity and lineage
//! - [`DocumentType`]: top-level format classification (image, audio, text, …)
//! - `document_format::*`: per-category leaf format enums
//!   ([`ImageFormat`], [`AudioFormat`], …) nested inside
//!   [`DocumentType`] variants

mod bundle;
mod content_data;
mod content_metadata;
mod document_format;
mod document_type;
mod encoding;

pub use nvisy_ontology::entity::ContentSource;

pub use self::bundle::Content;
pub use self::content_data::ContentData;
pub use self::content_metadata::{AnyAnnotations, ContentMetadata};
pub use self::document_format::{
    AudioFormat, ImageFormat, SpreadsheetFormat, TextFormat, WordFormat,
};
pub use self::document_type::DocumentType;
pub use self::encoding::TextEncoding;
