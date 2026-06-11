//! Content data containers, metadata, and source identity.
//!
//! - [`ContentData`]: raw content bytes with source identity.
//! - [`ContentDescriptor`]: caller-supplied descriptive bits
//!   (filename, MIME hint, extras) — built before bytes are
//!   persisted.
//! - [`ContentDigest`]: byte-derived facts (size, sha256, sniffed
//!   MIME) — computed at registration time.
//! - [`ContentRecord`]: persisted view (descriptor + digest), what
//!   registry reads return.
//! - [`Content`]: [`ContentData`] paired with an optional
//!   [`ContentDescriptor`] — the upload-shape carrier.
//! - [`ContentSource`]: UUIDv7-based content identity and lineage.
//!
//! Top-level format classification lives on [`FormatId`].
//!
//! [`FormatId`]: crate::FormatId

mod bundle;
mod content_data;
mod content_metadata;
mod encoding;

pub use nvisy_core::entity::ContentSource;

pub use self::bundle::Content;
pub use self::content_data::ContentData;
pub use self::content_metadata::{ContentDescriptor, ContentDigest, ContentRecord};
pub use self::encoding::TextEncoding;
