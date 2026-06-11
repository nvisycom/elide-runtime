//! What a codec handle exposes — the trait surface every shipped
//! format handler implements:
//!
//! - [`Handler`] — base trait every handler implements, regardless
//!   of modality. Identifies the format, exposes the content
//!   source, encodes back to bytes, surfaces embedded children for
//!   rich formats.
//! - [`Codable`] — codec-side extension of [`Modality`]: pins each
//!   modality marker to a runtime [`ModalityKind`] tag so erased
//!   handles can dispatch.
//! - [`Handle<M>`] — per-modality capability trait. Streaming
//!   ([`next_chunk`]), random-access reads and redactions
//!   ([`read`], [`redact`]), and offset lifting back to source
//!   coordinates ([`lift_chunk`]).
//! - [`Chunk<M>`] — one decoded unit yielded by `next_chunk`.
//! - [`HandleId`] — stable identifier for an embedded child
//!   handle, recorded on the parent's chunks via [`Chunk::embed`].
//! - [`EmbeddedHandles`] — rich-format capability: a parent handler
//!   resolves embed ids to [`UntypedDocumentHandle`]s on demand.
//!
//! [`Modality`]: nvisy_core::modality::Modality
//! [`UntypedDocumentHandle`]: crate::document::UntypedDocumentHandle
//! [`next_chunk`]: Handle::next_chunk
//! [`read`]: Handle::read
//! [`redact`]: Handle::redact
//! [`lift_chunk`]: Handle::lift_chunk

use std::fmt;
use std::ops::Range;

use nvisy_core::Error;
use nvisy_core::modality::Modality;
use nvisy_core::redaction::Redactions;
use uuid::Uuid;

use super::{FormatId, ModalityKind};
use crate::content::{ContentData, ContentSource};
use crate::document::UntypedDocumentHandle;

/// Base trait implemented by all format handlers, independent of
/// modality.
///
/// A handler holds loaded, validated content and provides methods
/// to identify and serialize it. Handlers are produced by their
/// corresponding [`Loader`].
///
/// Per-modality capability is provided by implementing
/// [`Handle<M>`] for the single modality the handler exposes.
/// Multi-modality is **not** done via multiple `Handle<M>` impls
/// on the same struct — rich formats implement [`EmbeddedHandles`]
/// and expose child handles instead.
///
/// [`Loader`]: super::Loader
pub trait Handler: Send + Sync + 'static {
    /// Stable id of the format this handler represents (e.g.
    /// `"nvisy.text.txt"`). Cheap to clone.
    fn format(&self) -> FormatId;

    /// Content source identity and lineage for this handler.
    fn source(&self) -> ContentSource;

    /// Serialize the current handler content back to [`ContentData`].
    fn encode(&self) -> Result<ContentData, Error>;

    /// Embedded-child accessor for rich formats (PDF, DOCX) whose
    /// chunks reference inner [`UntypedDocumentHandle`] handles.
    /// Returns `None` for leaf formats — the default — so only rich
    /// handlers need to override.
    ///
    /// The engine importer walks this to build the embed tree under
    /// the root document, without an `Any` downcast at the call site.
    fn embedded(&self) -> Option<&dyn EmbeddedHandles> {
        None
    }
}

/// Codec-side extension of [`Modality`]: pins each modality marker
/// to a runtime [`ModalityKind`] tag so the registry can erase
/// typed handles into the matching [`UntypedDocumentHandle`]
/// variant.
///
/// The per-location data payload yielded inside [`Chunk::data`]
/// and returned by [`Handle::read`] is [`Modality::Data`] — the
/// codec doesn't redefine it.
pub trait Codable: Modality {
    /// Runtime tag for this modality.
    const KIND: ModalityKind;
}

/// Stable identifier for an embedded child handle inside a rich
/// document. Issued by the producing handler at decode time and
/// recorded on the parent's chunks via [`Chunk::embed`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct HandleId(Uuid);

impl HandleId {
    /// Fresh identifier for a newly decoded embedded handle.
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Wrap a caller-provided UUID. Useful for round-tripping
    /// handles across a stable boundary (snapshot/restore,
    /// distributed pipelines).
    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }

    /// Inner UUID.
    pub fn as_uuid(&self) -> Uuid {
        self.0
    }
}

impl Default for HandleId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for HandleId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// One decoded unit yielded by [`Handle::next_chunk`].
///
/// `data` is the per-modality wire payload; `location` is the
/// coordinate the handler will accept in [`Handle::read`] /
/// [`Handle::redact`] to address the same chunk again.
///
/// `embed` is `Some(id)` only for text chunks that reference an
/// embedded child handle (e.g. an image figure in a PDF); resolve
/// it through [`EmbeddedHandles::get`] on the parent handler.
/// Non-text modalities leave it `None`.
#[derive(Debug, Clone, PartialEq)]
pub struct Chunk<M: Codable> {
    /// Coordinate addressing this chunk inside the handler.
    pub location: M::Location,
    /// Wire payload at the chunk's location.
    pub data: M::Data,
    /// Reference to an embedded child handle, if any.
    pub embed: Option<HandleId>,
}

/// Per-modality capability trait every format handler implements:
/// streaming chunks ([`next_chunk`]), random-access reads and
/// redactions ([`read`], [`redact`]), and offset lifting back to
/// source coordinates ([`lift_chunk`]).
///
/// The handler owns the streaming cursor — concurrent iteration
/// of the same handle is not supported (only one `&mut self`).
///
/// [`next_chunk`]: Handle::next_chunk
/// [`read`]: Handle::read
/// [`redact`]: Handle::redact
/// [`lift_chunk`]: Handle::lift_chunk
#[async_trait::async_trait]
pub trait Handle<M: Codable>: Handler {
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
    /// [`next_chunk`]: Handle::next_chunk
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

/// Capability trait for rich-format handlers (PDF, DOCX, …) whose
/// chunks reference embedded child handles.
///
/// A primary handler that emits [`Chunk`]s with [`Chunk::embed`]
/// set implements this trait so consumers can resolve the
/// referenced handle on demand. Embedded handles are
/// **first-class** [`UntypedDocumentHandle`]s — they carry their
/// own source identity, format, and cursor.
///
/// Lookup is lazy: implementations decode the referenced child on
/// demand, so a 500-page PDF with 2000 images does not pay the
/// extraction cost until a consumer asks for a specific image.
pub trait EmbeddedHandles: Send + Sync {
    /// Resolve an embedded child handle. Returns `None` if the id
    /// was never issued by this handler or the child can no longer
    /// be produced.
    fn get(&self, id: HandleId) -> Option<UntypedDocumentHandle>;
}
