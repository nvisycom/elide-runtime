//! Content data containers, metadata, and source identity.
//!
//! - [`ContentData`]: Raw content bytes with source identity
//! - [`ContentMetadata`]: MIME type, filename, and descriptive attributes
//! - [`Content`]: [`ContentData`] paired with optional [`ContentMetadata`]
//! - [`ContentSource`]: UUIDv7-based content identity and lineage
//!
//! Top-level format classification lives on [`FormatId`] alongside in
//! [`crate::core`].
//!
//! [`FormatId`]: crate::core::FormatId

mod bundle;
mod content_data;
mod content_metadata;
mod encoding;

pub use nvisy_core::entity::ContentSource;

pub use self::bundle::Content;
pub use self::content_data::ContentData;
pub use self::content_metadata::ContentMetadata;
pub use self::encoding::TextEncoding;
