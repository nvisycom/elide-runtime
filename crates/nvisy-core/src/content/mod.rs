//! Content data containers, metadata, source identity, and small
//! per-modality format tags.
//!
//! - [`ContentData`]: Raw content bytes with source identity
//! - [`ContentMetadata`]: MIME type, filename, and descriptive attributes
//! - [`Content`]: [`ContentData`] paired with optional [`ContentMetadata`]
//! - [`ContentSource`]: UUIDv7-based content identity and lineage
//! - [`ImageFormat`] / [`AudioFormat`]: small per-modality format tags
//!   used outside the codec layer (notably the OCR backend wire shape)
//!
//! Top-level format classification lives on [`FormatId`] in
//! `nvisy-codec`.
//!
//! [`FormatId`]: nvisy_codec::FormatId

mod bundle;
mod content_data;
mod content_metadata;
mod document_format;
mod encoding;

pub use self::bundle::Content;
pub use self::content_data::ContentData;
pub use self::content_metadata::{AnyAnnotations, ContentMetadata};
pub use self::document_format::{AudioFormat, ImageFormat};
pub use self::encoding::TextEncoding;
pub use crate::entity::ContentSource;
