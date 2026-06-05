//! Per-modality codec contracts: [`Codable`], [`Handle<M>`],
//! [`IndexedHandle<M>`], [`EmbeddedHandles`], [`Chunk<M>`], [`HandleId`].
//!
//! [`Codable`] extends [`Modality`] with the wire types the codec
//! exchanges for that modality — the per-location `Data` payload
//! ([`TextData`] / [`ImageData`] / [`AudioData`]) and the
//! per-location `Redaction` instruction
//! ([`TextRedaction`] / [`ImageRedaction`] / …).
//!
//! [`Handle<M>`] is the **streaming-default** per-modality capability
//! trait every format handler implements. It exposes a single
//! [`next_chunk`] method: handlers yield decoded
//! `(location, data, [embed])` chunks as they advance through the
//! underlying bytes. The handler owns the cursor.
//!
//! [`IndexedHandle<M>`] is the **random-access** super-trait formats
//! implement when they can address locations directly (text, image,
//! audio, tabular). Streaming-only formats (encrypted streams,
//! append-only logs) implement only [`Handle<M>`].
//!
//! [`EmbeddedHandles`] is the **rich-format capability**: a text
//! handler whose chunks reference embedded image (or other modality)
//! handles exposes them via [`EmbeddedHandles::get`]. Each embedded
//! handle is a first-class [`UntypedDocumentHandle`] with its own
//! source identity and cursor — the rich handler does **not**
//! implement multiple `Handle<M>` traits itself.
//!
//! [`TextData`]: crate::core::TextData
//! [`ImageData`]: crate::core::ImageData
//! [`AudioData`]: crate::core::AudioData
//! [`TextRedaction`]: crate::core::TextRedaction
//! [`ImageRedaction`]: crate::core::ImageRedaction
//! [`next_chunk`]: Handle::next_chunk
//! [`UntypedDocumentHandle`]: crate::document::UntypedDocumentHandle
//! [`Modality`]: nvisy_core::modality::Modality

use std::fmt;

use nvisy_core::Error;
use nvisy_core::modality::{ModalityData, ModalityKind};
use nvisy_core::redaction::{Redactable, Redactions};
use uuid::Uuid;

use crate::core::Handler;

/// Codec-side extension of [`ModalityData`]: adds the per-location
/// redaction instruction the codec applies, and the runtime tag the
/// registry uses to erase typed handles.
///
/// The per-location data payload yielded inside [`Chunk::data`] and
/// returned by [`IndexedHandle::read`] is [`ModalityData::Data`] —
/// the codec doesn't redefine it.
pub trait Codable: ModalityData + Redactable {
    /// Runtime tag for this modality. Used by the codec registry to
    /// erase a typed [`crate::DocumentHandle`] into an
    /// [`UntypedDocumentHandle`] variant.
    ///
    /// [`UntypedDocumentHandle`]: crate::document::UntypedDocumentHandle
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

    /// Wrap a caller-provided UUID. Useful for round-tripping handles
    /// across a stable boundary (snapshot/restore, distributed
    /// pipelines).
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
/// coordinate the handler will accept in [`IndexedHandle::read`] /
/// [`IndexedHandle::redact`] to address the same chunk again.
///
/// `embed` is `Some(id)` only for text chunks that reference an
/// embedded child handle (e.g. an image figure in a PDF); resolve it
/// through [`EmbeddedHandles::get`] on the parent handler. Non-text
/// modalities leave it `None`.
#[derive(Debug, Clone, PartialEq)]
pub struct Chunk<M: Codable> {
    /// Coordinate addressing this chunk inside the handler.
    pub location: M::Location,
    /// Wire payload at the chunk's location.
    pub data: M::Data,
    /// Reference to an embedded child handle, if any.
    pub embed: Option<HandleId>,
}

/// Per-modality streaming capability trait. Every format handler that
/// exposes modality `M` implements this.
///
/// `next_chunk` advances the handler's internal cursor and returns
/// the next decoded chunk, or `None` at end-of-stream. The handler
/// owns the cursor — concurrent iteration of the same handle is not
/// supported (there is only one `&mut self`).
#[async_trait::async_trait]
pub trait Handle<M: Codable>: Handler {
    /// Advance the cursor and yield the next chunk, or `None` at
    /// end-of-stream.
    async fn next_chunk(&mut self) -> Result<Option<Chunk<M>>, Error>;
}

/// Random-access capability trait for formats that can address
/// locations directly (the common case: text, image, audio, tabular).
///
/// Streaming-only formats omit this impl; consumers fall back to
/// driving [`Handle::next_chunk`] and buffering as needed.
#[async_trait::async_trait]
pub trait IndexedHandle<M: Codable>: Handle<M> {
    /// Read the wire payload at the given location. Used by
    /// [`TextAt`] resolvers to fetch bytes for a coordinate already
    /// known from somewhere else (an entity audit record, an
    /// annotation). Extraction itself does not call this — it drives
    /// [`Handle::next_chunk`] which returns `(location, data)`
    /// together.
    ///
    /// [`TextAt`]: nvisy_core::TextAt
    async fn read(&self, location: &M::Location) -> Result<Option<M::Data>, Error>;

    /// Apply a batch of `(location, replacement)` pairs in whatever
    /// order is correct for this format. Engine guarantees no two
    /// locations overlap; handler decides ordering (right-to-left
    /// for text/audio so deletions don't shift later indices, batch
    /// per page for PDF, …). The first error aborts the batch.
    ///
    /// Use [`Redactions::single`] when only one replacement is needed.
    async fn redact(&mut self, redactions: Redactions<M>) -> Result<(), Error>;
}

/// Capability trait for rich-format handlers (PDF, DOCX, …) whose
/// chunks reference embedded child handles.
///
/// A primary handler that emits [`Chunk`]s with
/// [`Chunk::embed`] set implements this trait so consumers can
/// resolve the referenced handle on demand. Embedded handles are
/// **first-class** [`UntypedDocumentHandle`]s — they carry their
/// own source identity, format, and cursor.
///
/// Lookup is lazy: implementations decode the referenced child on
/// demand, so a 500-page PDF with 2000 images does not pay the
/// extraction cost until a consumer asks for a specific image.
///
/// [`UntypedDocumentHandle`]: crate::document::UntypedDocumentHandle
pub trait EmbeddedHandles: Send + Sync {
    /// Resolve an embedded child handle. Returns `None` if the id was
    /// never issued by this handler or the child can no longer be
    /// produced.
    fn get(&self, id: HandleId) -> Option<crate::document::UntypedDocumentHandle>;
}
