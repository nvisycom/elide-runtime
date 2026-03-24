//! Content data containers, metadata, and source identity.
//!
//! - [`ContentData`]: Raw content bytes with source identity
//! - [`ContentMetadata`]: MIME type, filename, and descriptive attributes
//! - [`Content`]: [`ContentData`] paired with optional [`ContentMetadata`]
//! - [`ContentSource`]: UUIDv7-based content identity and lineage
//! - [`DataReference`]: Lightweight pointer into a content source

mod bundle;
mod content_bytes;
mod content_data;
mod content_metadata;
mod data_reference;
mod encoding;
mod source;

pub use self::bundle::Content;
pub use self::content_bytes::ContentBytes;
pub use self::content_data::ContentData;
pub use self::content_metadata::ContentMetadata;
pub use self::data_reference::DataReference;
pub use self::encoding::TextEncoding;
pub use self::source::ContentSource;
