//! What a codec handler exposes — the trait surface every shipped
//! format handler implements:
//!
//! - [`Handler<M>`] — per-modality capability trait. Identifies and
//!   serialises the handler ([`format`], [`source`], [`encode`]),
//!   streams chunks ([`next_chunk`]), supports random-access reads
//!   and redactions ([`read`], [`redact`]), and lifts recognizer
//!   offsets back to source coordinates ([`lift_chunk`]).
//! - [`Chunk<M>`] — one decoded unit yielded by `next_chunk`.
//!
//! [`format`]: Handler::format
//! [`source`]: Handler::source
//! [`encode`]: Handler::encode
//! [`next_chunk`]: Handler::next_chunk
//! [`read`]: Handler::read
//! [`redact`]: Handler::redact
//! [`lift_chunk`]: Handler::lift_chunk

use std::ops::Range;

use nvisy_core::Error;
use nvisy_core::modality::Modality;
use nvisy_core::redaction::Redactions;

use super::FormatId;
use crate::content::{ContentData, ContentSource};

/// One decoded unit yielded by [`Handler::next_chunk`].
///
/// `data` is the per-modality wire payload; `location` is the
/// coordinate the handler will accept in [`Handler::read`] /
/// [`Handler::redact`] to address the same chunk again.
#[derive(Debug, Clone, PartialEq)]
pub struct Chunk<M: Modality> {
    /// Coordinate addressing this chunk inside the handler.
    pub location: M::Location,
    /// Wire payload at the chunk's location.
    pub data: M::Data,
}

/// Per-modality capability trait every format handler implements.
///
/// Identifies and serialises the handler ([`format`], [`source`],
/// [`encode`]), streams chunks ([`next_chunk`]), supports
/// random-access reads and redactions ([`read`], [`redact`]), and
/// lifts recognizer offsets back to source coordinates
/// ([`lift_chunk`]).
///
/// The handler owns the streaming cursor — concurrent iteration
/// of the same handle is not supported (only one `&mut self`).
///
/// [`format`]: Handler::format
/// [`source`]: Handler::source
/// [`encode`]: Handler::encode
/// [`next_chunk`]: Handler::next_chunk
/// [`read`]: Handler::read
/// [`redact`]: Handler::redact
/// [`lift_chunk`]: Handler::lift_chunk
#[async_trait::async_trait]
pub trait Handler<M: Modality>: Send + Sync + 'static {
    /// Stable id of the format this handler represents (e.g.
    /// `"nvisy.text.txt"`). Cheap to clone.
    fn format(&self) -> FormatId;

    /// Content source identity and lineage for this handler.
    fn source(&self) -> ContentSource;

    /// Serialize the current handler content back to [`ContentData`].
    fn encode(&self) -> Result<ContentData, Error>;

    /// Advance the cursor and yield the next chunk, or `None` at
    /// end-of-stream.
    async fn next_chunk(&mut self) -> Result<Option<Chunk<M>>, Error>;

    /// Read the wire payload at the given location. Used by
    /// [`TextAt`] resolvers to fetch bytes for a coordinate already
    /// known from somewhere else (an entity audit record, an
    /// annotation). Extraction itself does not call this — it
    /// drives [`next_chunk`] which returns `(location, data)`
    /// together.
    ///
    /// [`next_chunk`]: Handler::next_chunk
    /// [`TextAt`]: nvisy_core::extraction::TextAt
    async fn read(&self, location: &M::Location) -> Result<Option<M::Data>, Error>;

    /// Apply a batch of `(location, replacement)` pairs in whatever
    /// order is correct for this format. Engine guarantees no two
    /// locations overlap; handler decides ordering (right-to-left
    /// for text/audio so deletions don't shift later indices, batch
    /// per page for PDF, …). The first error aborts the batch.
    ///
    /// Use [`Redactions::single`] when only one replacement is needed.
    async fn redact(&mut self, redactions: Redactions<M>) -> Result<(), Error>;

    /// Translate a `value_range` expressed inside `chunk.data`'s
    /// coordinate system into a source-coordinate `M::Location`.
    ///
    /// Recognizers see the unescaped, decoded chunk payload and
    /// emit offsets into that. Downstream stages — dedup, redact,
    /// audit — need locations that address the handler's source
    /// bytes. `lift_chunk` is the bridge.
    ///
    /// For text-shaped handlers where `chunk.data` is byte-for-byte
    /// a slice of source (TXT lines, HTML text nodes, PDF page
    /// text, CSV cells, DOCX text runs), the mapping is the
    /// identity offset add against `chunk.location.start`. Handlers
    /// whose chunks decode escapes or otherwise transform the
    /// payload (JSON `\"` / `\\`, future HTML entity refs) override
    /// to walk their per-chunk escape map.
    ///
    /// Returns `None` when the range has no source pre-image — out
    /// of bounds, lands inside an escape pair, or the modality
    /// doesn't have a meaningful `usize` value-range concept (image
    /// bounding boxes, audio time spans, tabular cell coords).
    /// Non-text impls leave the default `None`.
    fn lift_chunk(&self, _chunk: &Chunk<M>, _value_range: Range<usize>) -> Option<M::Location> {
        None
    }
}
