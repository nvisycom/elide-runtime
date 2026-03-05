//! Content metadata and classification types.
//!
//! # Core Types
//!
//! - [`ContentMetadata`]: Metadata information for content files
//! - [`ContentKind`]: Classification of content types by file extension
//! - [`DocumentType`]: Document classification details

mod content_kind;
mod content_metadata;
mod document_type;

pub use content_kind::ContentKind;
pub use content_metadata::ContentMetadata;
pub use document_type::DocumentType;
