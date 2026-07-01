//! [`RawDocument`]: in-memory carrier for a decoder input.

use bytes::Bytes;
use hipstr::HipStr;

/// A document ready for the codec: raw bytes plus the hints the
/// codec needs to resolve a decoder.
///
/// In-memory carrier — not persisted, not on the wire. Built from
/// a [`FileMetadata`] + its bytes when the engine reads a stored
/// file, or from an upload's body + headers at ingest.
///
/// [`FileMetadata`]: super::FileMetadata
#[derive(Debug, Clone)]
pub struct RawDocument {
    /// Raw file bytes.
    pub bytes: Bytes,
    /// File extension the codec registry resolves on (e.g.
    /// `"txt"`, `"pdf"`, `"png"`). Case-insensitive, no leading
    /// dot.
    pub extension: HipStr<'static>,
    /// Caller-supplied MIME hint (e.g. `application/pdf`). The
    /// codec uses [`extension`], not this; recorded for audit and
    /// for clients that round-trip metadata.
    ///
    /// [`extension`]: RawDocument::extension
    pub content_type: Option<HipStr<'static>>,
}
