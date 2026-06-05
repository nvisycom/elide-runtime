//! Codec core contracts: per-modality trait surfaces, format identity,
//! the registry that composes them, and the supporting types those
//! traits reference.
//!
//! - [`Codable`] — per-modality wire-type associated types.
//! - [`Handle<M>`] — streaming-default per-modality capability trait.
//! - [`IndexedHandle<M>`] — random-access super-trait.
//! - [`EmbeddedHandles`] — rich-format embedded-child lookup.
//! - [`Chunk<M>`] — one unit yielded by [`Handle::next_chunk`].
//! - [`HandleId`] — stable identifier for embedded child handles.
//! - [`FormatId`] — stable identifier for a registered format.
//! - [`CodecRegistry`] — extension/content-type → [`Format`] lookup +
//!   decode dispatch.
//! - [`Format`] — descriptor a [`CodecRegistry`] indexes.
//! - [`ErasedLoader`] + [`LoaderAdapter`] — object-safe loader surface
//!   adapting per-modality [`Loader<M>`] impls.
//! - [`WrapUntyped`] — modality-specific erase into
//!   [`UntypedDocumentHandle`].
//! - [`Redactions<S, R>`] — `(location, redaction)` pair list handed
//!   from the engine to a codec.
//!
//! Base traits ([`Handler`], [`Loader<M>`]) live in
//! [`crate::handler`], next to the concrete per-modality wire types
//! implementing them. Concrete per-modality wire types (`TextData`,
//! `TextRedaction`, `ImageData`, …) live in [`crate::handler`];
//! concrete format handlers live in `nvisy-formats`.
//!
//! [`Handler`]: crate::handler::Handler
//! [`Loader<M>`]: crate::handler::Loader
//! [`UntypedDocumentHandle`]: crate::document::UntypedDocumentHandle

mod format;
mod handle;
mod redactions;
mod registry;

pub use self::format::FormatId;
pub use self::handle::{Chunk, Codable, EmbeddedHandles, Handle, HandleId, IndexedHandle};
pub use self::redactions::Redactions;
pub use self::registry::{CodecRegistry, ErasedLoader, Format, LoaderAdapter, WrapUntyped};
